/// Service layer with Tokio stack integration using Axum
use std::sync::Arc;
use std::time::Duration;
use tokio::task::JoinHandle;
use tracing::{info, error};
use axum::{
    routing::{get, Router},
    extract::State,
    http::StatusCode,
    response::Json,
};
use tower_http::{
    trace::TraceLayer,
    timeout::TimeoutLayer,
};
use anyhow::Result;
use serde_json::json;

use crate::{SigmaEngineBuilder, Result as SigmaResult};

/// Main Sigma service with full Tokio integration
pub struct SigmaService {
    engine: Arc<SigmaEngine>,
    health_server: Option<JoinHandle<()>>,
    metrics_server: Option<JoinHandle<()>>,
    grpc_server: Option<JoinHandle<()>>,
    kafka_consumer: Option<JoinHandle<()>>,
}

impl SigmaService {
    /// Create a new service from builder configuration
    pub async fn new(builder: SigmaEngineBuilder) -> SigmaResult<Self> {
        // Initialize tracing
        crate::init_tracing();
        info!("Starting Sigma service v{}", crate::VERSION);
        
        // Build the core engine
        let engine = Arc::new(SigmaEngine::new(builder.clone()).await?);
        
        // Start health check server
        let health_server = Some(tokio::spawn(Self::start_health_server(engine.clone())));
        
        // Start metrics server
        let metrics_server = Some(tokio::spawn(Self::start_metrics_server(engine.clone())));
        
        // Start gRPC control plane
        let grpc_server = Some(tokio::spawn(Self::start_grpc_server(engine.clone())));
        
        // Start Kafka consumer if configured
        let kafka_consumer = if builder.kafka_config.is_some() {
            Some(tokio::spawn(Self::start_kafka_consumer(engine.clone(), builder)))
        } else {
            None
        };
        
        Ok(Self {
            engine,
            health_server,
            metrics_server,
            grpc_server,
            kafka_consumer,
        })
    }
    
    /// Run the service until shutdown
    pub async fn run(self) -> Result<()> {
        info!("Sigma service running");
        
        // Wait for shutdown signal
        tokio::signal::ctrl_c().await?;
        info!("Shutdown signal received");
        
        // Graceful shutdown
        self.shutdown().await?;
        Ok(())
    }
    
    /// Graceful shutdown
    async fn shutdown(self) -> Result<()> {
        info!("Shutting down services");
        
        // Cancel all tasks
        if let Some(handle) = self.health_server {
            handle.abort();
        }
        if let Some(handle) = self.metrics_server {
            handle.abort();
        }
        if let Some(handle) = self.grpc_server {
            handle.abort();
        }
        if let Some(handle) = self.kafka_consumer {
            handle.abort();
        }
        
        Ok(())
    }
    
    /// Start health check HTTP server using Axum
    async fn start_health_server(engine: Arc<SigmaEngine>) -> () {
        let app = Router::new()
            .route("/health", get(health_handler))
            .route("/ready", get(ready_handler))
            .layer(TraceLayer::new_for_http())
            .layer(TimeoutLayer::new(Duration::from_secs(30)))
            .with_state(engine);
        
        let addr = "0.0.0.0:8080";
        let listener = match tokio::net::TcpListener::bind(addr).await {
            Ok(l) => l,
            Err(e) => {
                error!("Failed to bind health server: {}", e);
                return;
            }
        };
        
        info!("Health server listening on http://{}", addr);
        
        if let Err(e) = axum::serve(listener, app).await {
            error!("Health server error: {}", e);
        }
    }
    
    /// Start metrics HTTP server using Axum
    async fn start_metrics_server(engine: Arc<SigmaEngine>) -> () {
        let app = Router::new()
            .route("/metrics", get(metrics_handler))
            .layer(TraceLayer::new_for_http())
            .layer(TimeoutLayer::new(Duration::from_secs(30)))
            .with_state(engine);
        
        let addr = "0.0.0.0:9090";
        let listener = match tokio::net::TcpListener::bind(addr).await {
            Ok(l) => l,
            Err(e) => {
                error!("Failed to bind metrics server: {}", e);
                return;
            }
        };
        
        info!("Metrics server listening on http://{}", addr);
        
        if let Err(e) = axum::serve(listener, app).await {
            error!("Metrics server error: {}", e);
        }
    }
    
    /// Start gRPC control plane server
    async fn start_grpc_server(_engine: Arc<SigmaEngine>) -> () {
        // This will be implemented when we add the proto definitions
        info!("gRPC server starting on port 50051");
        // Placeholder for now
        tokio::time::sleep(Duration::from_secs(3600)).await;
    }
    
    /// Start Kafka consumer
    async fn start_kafka_consumer(engine: Arc<SigmaEngine>, builder: SigmaEngineBuilder) -> () {
        if let Some(kafka_config) = builder.kafka_config {
            info!("Starting Kafka consumer for topics: {:?}", kafka_config.topics);
            
            // Create consumer configuration
            let mut consumer_config = crate::consumer::ConsumerConfig::builder()
                .brokers(kafka_config.brokers)
                .group_id(kafka_config.group_id)
                .topics(kafka_config.topics);
            
            // Apply optional configurations
            if let Some(batch_size) = kafka_config.batch_size {
                consumer_config = consumer_config.batch_size(batch_size);
            }
            
            if let Some(max_retries) = kafka_config.max_retries {
                consumer_config = consumer_config.max_retries(max_retries);
            }
            
            if let Some(dlq_topic) = kafka_config.dlq_topic {
                consumer_config = consumer_config.dlq_topic(dlq_topic);
            }
            
            if let Some(buffer_size) = kafka_config.backpressure_buffer_size {
                consumer_config = consumer_config.channel_buffer_size(buffer_size);
            }
            
            // Add any additional Kafka properties
            for (key, value) in kafka_config.properties {
                consumer_config = consumer_config.kafka_property(key, value);
            }
            
            let config = consumer_config.build();
            
            // Create the consumer
            match crate::consumer::create_sigma_consumer(engine, config).await {
                Ok(consumer) => {
                    info!("Successfully created Redpanda consumer");
                    if let Err(e) = consumer.run().await {
                        error!("Consumer error: {}", e);
                    }
                }
                Err(e) => {
                    error!("Failed to create consumer: {}", e);
                }
            }
        }
    }
}

/// Health check handler
async fn health_handler(State(engine): State<Arc<SigmaEngine>>) -> Result<Json<serde_json::Value>, StatusCode> {
    let status = engine.health_status();
    let body = json!({
        "status": if status.healthy { "ok" } else { "error" },
        "version": crate::VERSION,
        "rules_loaded": status.rules_loaded,
        "uptime_seconds": status.uptime_seconds,
    });
    
    if status.healthy {
        Ok(Json(body))
    } else {
        Err(StatusCode::SERVICE_UNAVAILABLE)
    }
}

/// Ready check handler
async fn ready_handler(State(engine): State<Arc<SigmaEngine>>) -> StatusCode {
    if engine.is_ready() {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    }
}

/// Metrics handler
async fn metrics_handler(State(engine): State<Arc<SigmaEngine>>) -> String {
    engine.collect_metrics()
}

/// Core Sigma engine
pub struct SigmaEngine {
    ruleset: Arc<RuleSet>,
    start_time: std::time::Instant,
}

impl SigmaEngine {
    /// Create a new engine
    async fn new(builder: SigmaEngineBuilder) -> SigmaResult<Self> {
        // Load rules
        let ruleset = Arc::new(RuleSet::load(&builder).await?);
        info!("Loaded {} rules", ruleset.len());
        
        Ok(Self {
            ruleset,
            start_time: std::time::Instant::now(),
        })
    }
    
    /// Process a single event
    pub async fn process_event(&self, event: crate::DynamicEvent) -> Result<()> {
        let _results = self.ruleset.evaluate(&event).await?;
        // Process results...
        Ok(())
    }
    
    /// Get health status
    fn health_status(&self) -> HealthStatus {
        HealthStatus {
            healthy: true,
            rules_loaded: self.ruleset.len(),
            uptime_seconds: self.start_time.elapsed().as_secs(),
        }
    }
    
    /// Check if service is ready
    fn is_ready(&self) -> bool {
        self.ruleset.len() > 0
    }
    
    /// Collect Prometheus metrics
    fn collect_metrics(&self) -> String {
        // Placeholder for metrics collection
        format!("# HELP sigma_rules_loaded Number of rules loaded\n# TYPE sigma_rules_loaded gauge\nsigma_rules_loaded {}\n", self.ruleset.len())
    }
}

/// Health status response
struct HealthStatus {
    healthy: bool,
    rules_loaded: usize,
    uptime_seconds: u64,
}

/// Ruleset placeholder
struct RuleSet {
    rules: Vec<String>, // Placeholder
}

impl RuleSet {
    async fn load(_builder: &SigmaEngineBuilder) -> SigmaResult<Self> {
        // Placeholder implementation
        Ok(Self {
            rules: vec!["rule1".to_string(), "rule2".to_string()],
        })
    }
    
    fn len(&self) -> usize {
        self.rules.len()
    }
    
    async fn evaluate(&self, _event: &crate::DynamicEvent) -> Result<Vec<String>> {
        // Placeholder implementation
        Ok(vec![])
    }
}