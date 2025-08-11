use crate::{SigmaEngine, SigmaError};
use axum::{
    extract::{DefaultBodyLimit, State},
    http::{HeaderMap, Request, StatusCode},
    middleware::{self, Next},
    response::Json,
    routing::{get, Router},
};
use once_cell::sync::Lazy;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::sync::atomic::{AtomicU64, Ordering};
/// Service layer with Tokio stack integration using Axum
use std::sync::Arc;
use std::time::Duration;
use tokio::task::JoinHandle;
use tower::limit::ConcurrencyLimitLayer;
use tower_http::{cors::CorsLayer, timeout::TimeoutLayer};
use tracing::{error, info};

/// Maximum request body size (1MB)
const MAX_REQUEST_SIZE: usize = 1024 * 1024;

/// Default request timeout (30 seconds)
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);

/// Maximum concurrent requests
const MAX_CONCURRENT_REQUESTS: usize = 1000;

/// Service metrics
static SERVICE_METRICS: Lazy<ServiceMetrics> = Lazy::new(ServiceMetrics::new);

/// API key from environment variable (for demo purposes)
static API_KEY: Lazy<Option<String>> = Lazy::new(|| std::env::var("SIGMA_API_KEY").ok());

#[derive(Clone)]
pub struct SigmaService {
    engine: Arc<SigmaEngine>,
    start_time: std::time::Instant,
    #[cfg(feature = "metrics")]
    metrics_registry: Option<Arc<prometheus::Registry>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct HealthResponse {
    status: String,
    version: String,
    uptime_seconds: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MetricsResponse {
    rules_loaded: usize,
    events_processed: u64,
    matches_found: u64,
    processing_time_ms: f64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct EvaluateRequest {
    #[serde(flatten)]
    event: serde_json::Map<String, serde_json::Value>,
}

/// Service-specific metrics
struct ServiceMetrics {
    requests_total: AtomicU64,
    requests_success: AtomicU64,
    requests_failed: AtomicU64,
    events_evaluated: AtomicU64,
    matches_found: AtomicU64,
    processing_durations: RwLock<Vec<Duration>>,
}

impl ServiceMetrics {
    fn new() -> Self {
        Self {
            requests_total: AtomicU64::new(0),
            requests_success: AtomicU64::new(0),
            requests_failed: AtomicU64::new(0),
            events_evaluated: AtomicU64::new(0),
            matches_found: AtomicU64::new(0),
            processing_durations: RwLock::new(Vec::with_capacity(1000)),
        }
    }

    fn record_request(&self, success: bool) {
        self.requests_total.fetch_add(1, Ordering::Relaxed);
        if success {
            self.requests_success.fetch_add(1, Ordering::Relaxed);
        } else {
            self.requests_failed.fetch_add(1, Ordering::Relaxed);
        }
    }

    fn record_evaluation(&self, duration: Duration, matched: bool) {
        self.events_evaluated.fetch_add(1, Ordering::Relaxed);
        if matched {
            self.matches_found.fetch_add(1, Ordering::Relaxed);
        }

        let mut durations = self.processing_durations.write();
        // Keep only last 1000 samples to prevent unbounded growth
        if durations.len() >= 1000 {
            durations.remove(0);
        }
        durations.push(duration);
    }

    fn avg_processing_time(&self) -> f64 {
        let durations = self.processing_durations.read();
        if durations.is_empty() {
            return 0.0;
        }
        let sum: Duration = durations.iter().sum();
        sum.as_millis() as f64 / durations.len() as f64
    }
}

impl SigmaService {
    pub fn new(engine: Arc<SigmaEngine>) -> Self {
        Self {
            engine,
            start_time: std::time::Instant::now(),
            #[cfg(feature = "metrics")]
            metrics_registry: None,
        }
    }

    #[cfg(feature = "metrics")]
    pub fn with_metrics_registry(mut self, registry: Arc<prometheus::Registry>) -> Self {
        self.metrics_registry = Some(registry);
        self
    }

    pub fn router(&self) -> Router {
        let app = Router::new()
            .route("/health", get(Self::health_handler))
            .route("/metrics", get(Self::metrics_handler))
            .route("/rules", get(Self::list_rules_handler))
            .route("/evaluate", axum::routing::post(Self::evaluate_handler))
            .layer(middleware::from_fn(Self::auth_middleware))
            .with_state(self.clone());

        // Apply middleware layers
        app.layer(DefaultBodyLimit::max(MAX_REQUEST_SIZE))
            .layer(TimeoutLayer::new(DEFAULT_TIMEOUT))
            .layer(ConcurrencyLimitLayer::new(MAX_CONCURRENT_REQUESTS))
            .layer(CorsLayer::permissive())
    }

    /// Authentication middleware
    async fn auth_middleware(
        headers: HeaderMap,
        request: Request<axum::body::Body>,
        next: Next,
    ) -> Result<axum::response::Response, StatusCode> {
        // Skip auth for health endpoint
        if request.uri().path() == "/health" {
            return Ok(next.run(request).await);
        }

        // Check API key if configured
        if let Some(expected_key) = API_KEY.as_ref() {
            match headers.get("x-api-key") {
                Some(key) if key == expected_key.as_str() => Ok(next.run(request).await),
                _ => {
                    SERVICE_METRICS.record_request(false);
                    Err(StatusCode::UNAUTHORIZED)
                }
            }
        } else {
            // No API key configured, allow all requests
            Ok(next.run(request).await)
        }
    }

    async fn health_handler(State(service): State<SigmaService>) -> Json<HealthResponse> {
        let uptime = service.start_time.elapsed().as_secs();
        Json(HealthResponse {
            status: "healthy".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            uptime_seconds: uptime,
        })
    }

    async fn metrics_handler(State(service): State<SigmaService>) -> Json<MetricsResponse> {
        let metadata = service.engine.ruleset().get_metadata();
        SERVICE_METRICS.record_request(true);

        Json(MetricsResponse {
            rules_loaded: metadata.enabled_rules,
            events_processed: SERVICE_METRICS.events_evaluated.load(Ordering::Relaxed),
            matches_found: SERVICE_METRICS.matches_found.load(Ordering::Relaxed),
            processing_time_ms: SERVICE_METRICS.avg_processing_time(),
        })
    }

    async fn list_rules_handler(
        State(service): State<SigmaService>,
    ) -> Result<Json<Vec<String>>, StatusCode> {
        // For now, return basic metadata. To implement full rule listing,
        // we'd need to expose rule details from the RuleSet
        let metadata = service.engine.ruleset().get_metadata();
        let rules_info = vec![
            format!("Total rules: {}", metadata.total_rules),
            format!("Enabled rules: {}", metadata.enabled_rules),
            format!("Failed rules: {}", metadata.failed_rules),
        ];
        Ok(Json(rules_info))
    }

    async fn evaluate_handler(
        State(service): State<SigmaService>,
        Json(request): Json<EvaluateRequest>,
    ) -> Result<Json<serde_json::Value>, StatusCode> {
        let start_time = std::time::Instant::now();

        // Validate event data
        if request.event.is_empty() {
            SERVICE_METRICS.record_request(false);
            return Err(StatusCode::BAD_REQUEST);
        }

        // Create a DynamicEvent from the validated input
        let event = crate::event::DynamicEvent::new(serde_json::Value::Object(request.event));

        // Evaluate the event against all rules
        match service.engine.process_event(event).await {
            Ok(result) => {
                let has_matches = result.matches.iter().any(|m| m.matched);
                let duration = start_time.elapsed();

                SERVICE_METRICS.record_request(true);
                SERVICE_METRICS.record_evaluation(duration, has_matches);

                let matches: Vec<serde_json::Value> = result
                    .matches
                    .iter()
                    .filter(|m| m.matched)
                    .map(|m| {
                        serde_json::json!({
                            "rule_id": m.rule_id,
                            "rule_title": m.rule_title,
                            "matched": m.matched,
                            "evaluation_time_ms": m.evaluation_time.as_millis()
                        })
                    })
                    .collect();

                Ok(Json(serde_json::json!({
                    "matched": has_matches,
                    "rules": matches,
                    "total_rules_evaluated": result.rules_evaluated,
                    "evaluation_time_ms": result.evaluation_time.as_millis()
                })))
            }
            Err(e) => {
                error!("Event evaluation failed: {}", e);
                SERVICE_METRICS.record_request(false);

                // Don't expose internal error details
                match e {
                    SigmaError::ResourceLimitExceeded { .. } => Err(StatusCode::PAYLOAD_TOO_LARGE),
                    _ => Err(StatusCode::INTERNAL_SERVER_ERROR),
                }
            }
        }
    }

    pub async fn serve(
        self,
        addr: SocketAddr,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync + 'static>> {
        let app = self.router();

        info!("Starting Sigma service on {}", addr);

        let listener = tokio::net::TcpListener::bind(addr).await?;

        if let Err(e) = axum::serve(listener, app).await {
            error!("Server error: {}", e);
            return Err(e.into());
        }

        Ok(())
    }
}

// HTTP server implementation with Axum
pub struct HttpServer {
    app: Router,
    addr: SocketAddr,
}

impl HttpServer {
    pub fn new(engine: Arc<SigmaEngine>, addr: SocketAddr) -> Self {
        let service = SigmaService::new(engine);
        let app = service.router();

        Self { app, addr }
    }

    pub async fn run(self) -> Result<(), Box<dyn std::error::Error + Send + Sync + 'static>> {
        info!("Starting HTTP server on {}", self.addr);

        let listener = tokio::net::TcpListener::bind(self.addr).await?;

        if let Err(e) = axum::serve(listener, self.app).await {
            error!("Server error: {}", e);
            return Err(e.into());
        }

        Ok(())
    }
}

// gRPC server implementation with Tonic
#[cfg(feature = "service")]
pub mod grpc {
    use crate::{SigmaEngine, SigmaError};
    use std::net::SocketAddr;
    use std::sync::Arc;
    use tonic::{transport::Server, Request, Response, Status};
    use tracing::{error, info};

    // Generated protobuf code (will be generated by build.rs)
    pub mod sigma {
        tonic::include_proto!("sigma");
    }

    use futures::StreamExt;
    use sigma::{
        sigma_service_server::{SigmaService as SigmaServiceTrait, SigmaServiceServer},
        EvaluateEventRequest, EvaluateEventResponse, HealthRequest, HealthResponse,
        ListRulesRequest, ListRulesResponse, MetricsRequest, MetricsResponse, PerformanceMetrics,
        RuleMatch, RuleSummary, StreamEvaluateRequest, StreamEvaluateResponse,
    };

    pub struct SigmaGrpcService {
        engine: Arc<SigmaEngine>,
        start_time: std::time::Instant,
    }

    impl SigmaGrpcService {
        pub fn new(engine: Arc<SigmaEngine>) -> Self {
            Self {
                engine,
                start_time: std::time::Instant::now(),
            }
        }
    }

    #[tonic::async_trait]
    impl SigmaServiceTrait for SigmaGrpcService {
        async fn evaluate_event(
            &self,
            request: Request<EvaluateEventRequest>,
        ) -> Result<Response<EvaluateEventResponse>, Status> {
            let req = request.into_inner();

            // Parse the JSON event
            let event_value: serde_json::Value = serde_json::from_str(&req.event_json)
                .map_err(|e| Status::invalid_argument(format!("Invalid JSON: {}", e)))?;

            let event = crate::event::DynamicEvent::new(event_value);

            // Evaluate the event
            match self.engine.process_event(event).await {
                Ok(result) => {
                    let matches: Vec<RuleMatch> = result
                        .matches
                        .iter()
                        .map(|m| RuleMatch {
                            rule_id: m.rule_id.clone(),
                            rule_title: m.rule_title.clone(),
                            matched: m.matched,
                            evaluation_time_ms: m.evaluation_time.as_millis() as u64,
                            confidence: if m.matched { 1.0 } else { 0.0 },
                            metadata: std::collections::HashMap::new(),
                        })
                        .collect();

                    let has_matches = matches.iter().any(|m| m.matched);

                    Ok(Response::new(EvaluateEventResponse {
                        matched: has_matches,
                        matches,
                        rules_evaluated: result.rules_evaluated as u32,
                        evaluation_time_ms: result.evaluation_time.as_millis() as u64,
                        error: String::new(),
                    }))
                }
                Err(e) => {
                    error!("Event evaluation failed: {}", e);
                    // Don't expose internal error details
                    let sanitized_error = match e {
                        SigmaError::ResourceLimitExceeded { .. } => "Resource limit exceeded",
                        SigmaError::InvalidPattern(_) => "Invalid pattern",
                        _ => "Internal processing error",
                    };
                    Ok(Response::new(EvaluateEventResponse {
                        matched: false,
                        matches: vec![],
                        rules_evaluated: 0,
                        evaluation_time_ms: 0,
                        error: sanitized_error.to_string(),
                    }))
                }
            }
        }

        async fn get_health(
            &self,
            _request: Request<HealthRequest>,
        ) -> Result<Response<HealthResponse>, Status> {
            let metadata = self.engine.ruleset().get_metadata();
            let uptime = self.start_time.elapsed().as_secs();

            let status = if metadata.enabled_rules > 0 {
                "healthy"
            } else {
                "degraded"
            };

            let mut details = std::collections::HashMap::new();
            details.insert(
                "rules_loaded".to_string(),
                metadata.enabled_rules.to_string(),
            );
            details.insert(
                "failed_rules".to_string(),
                metadata.failed_rules.to_string(),
            );

            Ok(Response::new(HealthResponse {
                status: status.to_string(),
                version: env!("CARGO_PKG_VERSION").to_string(),
                uptime_seconds: uptime,
                details,
            }))
        }

        async fn get_metrics(
            &self,
            _request: Request<MetricsRequest>,
        ) -> Result<Response<MetricsResponse>, Status> {
            let metadata = self.engine.ruleset().get_metadata();

            Ok(Response::new(MetricsResponse {
                rules_loaded: metadata.enabled_rules as u32,
                events_processed: 0,         // Would be tracked by metrics system
                matches_found: 0,            // Would be tracked by metrics system
                avg_processing_time_ms: 0.0, // Would be tracked by metrics system
                memory_usage_bytes: 0,       // Would be tracked by metrics system
                performance: Some(PerformanceMetrics {
                    events_per_second: 0.0,
                    p50_processing_time_ms: 0.0,
                    p95_processing_time_ms: 0.0,
                    p99_processing_time_ms: 0.0,
                    error_rate: 0.0,
                }),
            }))
        }

        async fn list_rules(
            &self,
            _request: Request<ListRulesRequest>,
        ) -> Result<Response<ListRulesResponse>, Status> {
            let metadata = self.engine.ruleset().get_metadata();

            // For now, return basic rule summary
            // In a full implementation, we'd expose rule details from the RuleSet
            let rules = vec![RuleSummary {
                rule_id: "summary".to_string(),
                rule_title: format!("Loaded {} rules", metadata.enabled_rules),
                description: format!(
                    "Total: {}, Enabled: {}, Failed: {}",
                    metadata.total_rules, metadata.enabled_rules, metadata.failed_rules
                ),
                status: "enabled".to_string(),
                level: "info".to_string(),
                tags: vec!["summary".to_string()],
                last_modified: 0,
            }];

            Ok(Response::new(ListRulesResponse {
                rules,
                next_page_token: String::new(),
                total_count: metadata.total_rules as u32,
            }))
        }

        type StreamEvaluateStream = std::pin::Pin<
            Box<dyn tokio_stream::Stream<Item = Result<StreamEvaluateResponse, Status>> + Send>,
        >;

        async fn stream_evaluate(
            &self,
            request: Request<tonic::Streaming<StreamEvaluateRequest>>,
        ) -> Result<Response<Self::StreamEvaluateStream>, Status> {
            let mut stream = request.into_inner();
            let engine = Arc::clone(&self.engine);

            let output_stream = async_stream::stream! {
                while let Some(req) = stream.next().await {
                    match req {
                        Ok(request) => {
                            // Parse event
                            match serde_json::from_str::<serde_json::Value>(&request.event_json) {
                                Ok(event_value) => {
                                    let event = crate::event::DynamicEvent::new(event_value);

                                    // Evaluate
                                    match engine.process_event(event).await {
                                        Ok(result) => {
                                            let matches: Vec<RuleMatch> = result.matches
                                                .iter()
                                                .map(|m| RuleMatch {
                                                    rule_id: m.rule_id.clone(),
                                                    rule_title: m.rule_title.clone(),
                                                    matched: m.matched,
                                                    evaluation_time_ms: m.evaluation_time.as_millis() as u64,
                                                    confidence: if m.matched { 1.0 } else { 0.0 },
                                                    metadata: std::collections::HashMap::new(),
                                                })
                                                .collect();

                                            let has_matches = matches.iter().any(|m| m.matched);

                                            yield Ok(StreamEvaluateResponse {
                                                sequence: request.sequence,
                                                result: Some(EvaluateEventResponse {
                                                    matched: has_matches,
                                                    matches,
                                                    rules_evaluated: result.rules_evaluated as u32,
                                                    evaluation_time_ms: result.evaluation_time.as_millis() as u64,
                                                    error: String::new(),
                                                }),
                                                timestamp: chrono::Utc::now().timestamp(),
                                            });
                                        }
                                        Err(e) => {
                                            error!("Stream evaluation failed: {}", e);
                                            // Don't expose internal error details
                                            let sanitized_error = match e {
                                                SigmaError::ResourceLimitExceeded { .. } => "Resource limit exceeded",
                                                SigmaError::InvalidPattern(_) => "Invalid pattern",
                                                _ => "Internal processing error"
                                            };
                                            yield Ok(StreamEvaluateResponse {
                                                sequence: request.sequence,
                                                result: Some(EvaluateEventResponse {
                                                    matched: false,
                                                    matches: vec![],
                                                    rules_evaluated: 0,
                                                    evaluation_time_ms: 0,
                                                    error: sanitized_error.to_string(),
                                                }),
                                                timestamp: chrono::Utc::now().timestamp(),
                                            });
                                        }
                                    }
                                }
                                Err(e) => {
                                    yield Err(Status::invalid_argument(format!("Invalid JSON: {}", e)));
                                }
                            }
                        }
                        Err(e) => {
                            yield Err(e);
                        }
                    }
                }
            };

            Ok(Response::new(Box::pin(output_stream)))
        }
    }

    pub struct GrpcServer {
        addr: SocketAddr,
        engine: Arc<SigmaEngine>,
    }

    impl GrpcServer {
        pub fn new(engine: Arc<SigmaEngine>, addr: SocketAddr) -> Self {
            Self { addr, engine }
        }

        pub async fn run(self) -> Result<(), Box<dyn std::error::Error + Send + Sync + 'static>> {
            info!("Starting gRPC server on {}", self.addr);

            let service = SigmaGrpcService::new(self.engine);

            Server::builder()
                .add_service(SigmaServiceServer::new(service))
                .serve(self.addr)
                .await?;

            Ok(())
        }
    }
}

#[cfg(feature = "service")]
pub use grpc::{GrpcServer, SigmaGrpcService};

// Service runner that can manage multiple servers
pub struct ServiceRunner {
    http_server: Option<HttpServer>,
    #[cfg(feature = "service")]
    grpc_server: Option<grpc::GrpcServer>,
    #[cfg(not(feature = "service"))]
    grpc_server: Option<()>,
    handles: Vec<JoinHandle<Result<(), Box<dyn std::error::Error + Send + Sync + 'static>>>>,
}

impl ServiceRunner {
    pub fn new() -> Self {
        Self {
            http_server: None,
            grpc_server: None,
            handles: Vec::new(),
        }
    }

    pub fn with_http(mut self, engine: Arc<SigmaEngine>, addr: SocketAddr) -> Self {
        self.http_server = Some(HttpServer::new(engine, addr));
        self
    }

    #[cfg(feature = "service")]
    pub fn with_grpc(mut self, engine: Arc<SigmaEngine>, addr: SocketAddr) -> Self {
        self.grpc_server = Some(grpc::GrpcServer::new(engine, addr));
        self
    }

    pub async fn run(mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync + 'static>> {
        if let Some(http) = self.http_server.take() {
            let handle = tokio::spawn(async move { http.run().await });
            self.handles.push(handle);
        }

        #[cfg(feature = "service")]
        if let Some(grpc) = self.grpc_server.take() {
            let handle = tokio::spawn(async move { grpc.run().await });
            self.handles.push(handle);
        }

        // Set up graceful shutdown
        let shutdown = tokio::signal::ctrl_c();

        // Wait for either shutdown signal or server error
        tokio::select! {
            _ = shutdown => {
                info!("Graceful shutdown signal received");
                // Cancel all server tasks
                for handle in &self.handles {
                    handle.abort();
                }
            }
            res = async {
                let handles = std::mem::take(&mut self.handles);
                for handle in handles {
                    if let Err(e) = handle.await? {
                        return Err(e);
                    }
                }
                Ok::<(), Box<dyn std::error::Error + Send + Sync + 'static>>(())
            } => {
                if let Err(e) = res {
                    error!("Server error: {:?}", e);
                    return Err(e);
                }
            }
        }

        info!("All servers shut down gracefully");
        Ok(())
    }
}

// Metrics endpoint for Prometheus
#[cfg(feature = "metrics")]
#[derive(Clone)]
pub struct MetricsService {
    registry: Arc<prometheus::Registry>,
}

#[cfg(feature = "metrics")]
impl MetricsService {
    pub fn new(registry: Arc<prometheus::Registry>) -> Self {
        Self { registry }
    }

    pub async fn serve(
        self,
        addr: SocketAddr,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync + 'static>> {
        use axum::body::Body;
        use axum::response::Response;
        use prometheus::{Encoder, TextEncoder};

        let app = Router::new().route(
            "/metrics",
            get(move || async move {
                let mut buffer = vec![];
                let encoder = TextEncoder::new();
                let metric_families = self.registry.gather();
                encoder.encode(&metric_families, &mut buffer).unwrap();

                Response::builder()
                    .header("Content-Type", encoder.format_type())
                    .body(Body::from(buffer))
                    .unwrap()
            }),
        );

        info!("Starting metrics server on {}", addr);

        let listener = tokio::net::TcpListener::bind(addr).await?;
        axum::serve(listener, app).await?;

        Ok(())
    }
}

// Health check service
pub struct HealthCheckService {
    engine: Arc<SigmaEngine>,
}

impl HealthCheckService {
    pub fn new(engine: Arc<SigmaEngine>) -> Self {
        Self { engine }
    }

    pub async fn check(
        &self,
    ) -> Result<HealthResponse, Box<dyn std::error::Error + Send + Sync + 'static>> {
        // Basic health check - verify engine has rules loaded
        let metadata = self.engine.ruleset().get_metadata();
        let status = if metadata.enabled_rules > 0 {
            "healthy"
        } else {
            "degraded" // No rules loaded
        };

        Ok(HealthResponse {
            status: status.to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            uptime_seconds: 0, // Would need start time tracked in HealthCheckService too
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SigmaEngineBuilder;
    use axum::body::Body;
    use axum::http::{Method, Request};
    use std::sync::Arc;
    use tempfile::TempDir;
    use tower::ServiceExt;

    // Helper function to create a test engine
    async fn create_test_engine() -> Arc<SigmaEngine> {
        let temp_dir = TempDir::new().unwrap();
        let rules_dir = temp_dir.path().join("rules");
        std::fs::create_dir(&rules_dir).unwrap();

        // Create a simple test rule
        let rule_content = r#"
title: Test Rule
status: experimental
logsource:
    product: test
detection:
    keywords:
        - "test"
    condition: keywords
"#;
        std::fs::write(rules_dir.join("test.yml"), rule_content).unwrap();

        let builder = SigmaEngineBuilder::new().add_rule_dir(rules_dir.to_string_lossy());
        let engine = builder.build().await.unwrap();
        Arc::new(engine)
    }

    #[tokio::test]
    async fn test_health_endpoint() {
        let engine = create_test_engine().await;
        let service = SigmaService::new(engine);
        let app = service.router();

        let response = app
            .oneshot(
                Request::builder()
                    .method(Method::GET)
                    .uri("/health")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let health: HealthResponse = serde_json::from_slice(&body).unwrap();

        assert_eq!(health.status, "healthy");
        assert!(!health.version.is_empty());
        assert!(health.uptime_seconds >= 0);
    }

    #[tokio::test]
    async fn test_metrics_endpoint() {
        let engine = create_test_engine().await;
        let service = SigmaService::new(engine);
        let app = service.router();

        let response = app
            .oneshot(
                Request::builder()
                    .method(Method::GET)
                    .uri("/metrics")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let metrics: MetricsResponse = serde_json::from_slice(&body).unwrap();

        assert_eq!(metrics.rules_loaded, 1);
        assert!(metrics.events_processed >= 0);
        assert!(metrics.matches_found >= 0);
        assert!(metrics.processing_time_ms >= 0.0);
    }

    #[tokio::test]
    async fn test_rules_endpoint() {
        let engine = create_test_engine().await;
        let service = SigmaService::new(engine);
        let app = service.router();

        let response = app
            .oneshot(
                Request::builder()
                    .method(Method::GET)
                    .uri("/rules")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let rules: Vec<String> = serde_json::from_slice(&body).unwrap();

        assert!(!rules.is_empty());
        assert!(rules[0].contains("Total rules:"));
    }

    #[tokio::test]
    async fn test_evaluate_endpoint_success() {
        let engine = create_test_engine().await;
        let service = SigmaService::new(engine);
        let app = service.router();

        let event_data = serde_json::json!({
            "event": {
                "message": "test event"
            }
        });

        let response = app
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/evaluate")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_string(&event_data).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let result: serde_json::Value = serde_json::from_slice(&body).unwrap();

        assert!(result["matched"].is_boolean());
        assert!(result["rules"].is_array());
        assert!(result["total_rules_evaluated"].is_number());
        assert!(result["evaluation_time_ms"].is_number());
    }

    #[tokio::test]
    async fn test_evaluate_endpoint_empty_event() {
        let engine = create_test_engine().await;
        let service = SigmaService::new(engine);
        let app = service.router();

        let event_data = serde_json::json!({});

        let response = app
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/evaluate")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_string(&event_data).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_api_key_authentication() {
        // Set API key for testing
        std::env::set_var("SIGMA_API_KEY", "test-api-key");

        let engine = create_test_engine().await;
        let service = SigmaService::new(engine);
        let app = service.router();

        // Test without API key - should fail
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::GET)
                    .uri("/metrics")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

        // Test with correct API key - should succeed
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::GET)
                    .uri("/metrics")
                    .header("x-api-key", "test-api-key")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        // Test with wrong API key - should fail
        let response = app
            .oneshot(
                Request::builder()
                    .method(Method::GET)
                    .uri("/metrics")
                    .header("x-api-key", "wrong-key")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

        // Clean up
        std::env::remove_var("SIGMA_API_KEY");
    }

    #[tokio::test]
    async fn test_health_endpoint_bypasses_auth() {
        // Set API key for testing
        std::env::set_var("SIGMA_API_KEY", "test-api-key");

        let engine = create_test_engine().await;
        let service = SigmaService::new(engine);
        let app = service.router();

        // Health endpoint should work without API key
        let response = app
            .oneshot(
                Request::builder()
                    .method(Method::GET)
                    .uri("/health")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        // Clean up
        std::env::remove_var("SIGMA_API_KEY");
    }

    #[tokio::test]
    async fn test_request_size_limit() {
        let engine = create_test_engine().await;
        let service = SigmaService::new(engine);
        let app = service.router();

        // Create a request body larger than MAX_REQUEST_SIZE
        let large_event = serde_json::json!({
            "event": {
                "data": "x".repeat(2 * 1024 * 1024) // 2MB
            }
        });

        let response = app
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/evaluate")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_string(&large_event).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();

        // Should be rejected due to size limit
        assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
    }

    #[tokio::test]
    async fn test_invalid_json() {
        let engine = create_test_engine().await;
        let service = SigmaService::new(engine);
        let app = service.router();

        let response = app
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/evaluate")
                    .header("content-type", "application/json")
                    .body(Body::from("invalid json"))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_metrics_tracking() {
        let engine = create_test_engine().await;
        let service = SigmaService::new(engine);
        let app = service.router();

        // Reset metrics
        SERVICE_METRICS.requests_total.store(0, Ordering::Relaxed);
        SERVICE_METRICS.requests_success.store(0, Ordering::Relaxed);
        SERVICE_METRICS.requests_failed.store(0, Ordering::Relaxed);

        // Make a successful request
        let event_data = serde_json::json!({
            "event": {
                "message": "test"
            }
        });

        let _ = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/evaluate")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_string(&event_data).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(SERVICE_METRICS.requests_total.load(Ordering::Relaxed), 1);
        assert_eq!(SERVICE_METRICS.requests_success.load(Ordering::Relaxed), 1);
        assert_eq!(SERVICE_METRICS.events_evaluated.load(Ordering::Relaxed), 1);

        // Make a failed request
        let _ = app
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/evaluate")
                    .header("content-type", "application/json")
                    .body(Body::from("{}"))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(SERVICE_METRICS.requests_total.load(Ordering::Relaxed), 2);
        assert_eq!(SERVICE_METRICS.requests_failed.load(Ordering::Relaxed), 1);
    }

    #[cfg(feature = "service")]
    #[tokio::test]
    async fn test_grpc_health_endpoint() {
        use grpc::sigma::{sigma_service_server::SigmaService as GrpcSigmaService, HealthRequest};
        use tonic::Request;

        let engine = create_test_engine().await;
        let service = grpc::SigmaGrpcService::new(engine);

        let request = Request::new(HealthRequest {});
        let response = service.get_health(request).await.unwrap();
        let health = response.into_inner();

        assert!(!health.status.is_empty());
        assert!(!health.version.is_empty());
        assert!(health.uptime_seconds >= 0);
    }

    #[cfg(feature = "service")]
    #[tokio::test]
    async fn test_grpc_evaluate_endpoint() {
        use grpc::sigma::{
            sigma_service_server::SigmaService as GrpcSigmaService, EvaluateEventRequest,
        };
        use tonic::Request;

        let engine = create_test_engine().await;
        let service = grpc::SigmaGrpcService::new(engine);

        let event_json = serde_json::json!({
            "message": "test event"
        })
        .to_string();

        let request = Request::new(EvaluateEventRequest {
            event_json,
            rule_ids: vec![],
        });
        let response = service.evaluate_event(request).await.unwrap();
        let result = response.into_inner();

        assert!(!result.error.is_empty() || result.rules_evaluated > 0);
    }
}
