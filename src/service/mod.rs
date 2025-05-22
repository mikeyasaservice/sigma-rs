/// Service layer with Tokio stack integration using Axum
use std::sync::Arc;
use tokio::task::JoinHandle;
use tracing::{info, error};
use axum::{
    routing::{get, Router},
    extract::State,
    http::StatusCode,
    response::Json,
};
use crate::SigmaEngine;
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;

#[derive(Clone)]
pub struct SigmaService {
    engine: Arc<SigmaEngine>,
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

impl SigmaService {
    pub fn new(engine: Arc<SigmaEngine>) -> Self {
        Self { engine }
    }

    pub fn router(&self) -> Router {
        Router::new()
            .route("/health", get(Self::health_handler))
            .route("/metrics", get(Self::metrics_handler))
            .route("/rules", get(Self::list_rules_handler))
            .route("/evaluate", axum::routing::post(Self::evaluate_handler))
            .with_state(self.clone())
    }

    async fn health_handler(State(_service): State<SigmaService>) -> Json<HealthResponse> {
        Json(HealthResponse {
            status: "healthy".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            uptime_seconds: 0, // TODO: Implement actual uptime tracking
        })
    }

    async fn metrics_handler(State(_service): State<SigmaService>) -> Json<MetricsResponse> {
        Json(MetricsResponse {
            rules_loaded: 0, // TODO: Get from engine
            events_processed: 0,
            matches_found: 0,
            processing_time_ms: 0.0,
        })
    }

    async fn list_rules_handler(State(_service): State<SigmaService>) -> Result<Json<Vec<String>>, StatusCode> {
        // TODO: Implement actual rule listing
        Ok(Json(vec![]))
    }

    async fn evaluate_handler(
        State(_service): State<SigmaService>,
        Json(_event): Json<serde_json::Value>,
    ) -> Result<Json<serde_json::Value>, StatusCode> {
        // TODO: Implement actual evaluation
        Ok(Json(serde_json::json!({
            "matched": false,
            "rules": []
        })))
    }

    pub async fn serve(self, addr: SocketAddr) -> Result<(), Box<dyn std::error::Error + Send + Sync + 'static>> {
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
pub struct GrpcServer {
    addr: SocketAddr,
    engine: Arc<SigmaEngine>,
}

#[cfg(feature = "service")]
impl GrpcServer {
    pub fn new(engine: Arc<SigmaEngine>, addr: SocketAddr) -> Self {
        Self { addr, engine }
    }

    pub async fn run(self) -> Result<(), Box<dyn std::error::Error + Send + Sync + 'static>> {
        
        info!("Starting gRPC server on {}", self.addr);
        
        // TODO: Implement actual gRPC service
        // This is a placeholder
        
        Ok(())
    }
}

// Service runner that can manage multiple servers
pub struct ServiceRunner {
    http_server: Option<HttpServer>,
    grpc_server: Option<GrpcServer>,
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
        self.grpc_server = Some(GrpcServer::new(engine, addr));
        self
    }

    pub async fn run(mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync + 'static>> {
        if let Some(http) = self.http_server.take() {
            let handle = tokio::spawn(async move {
                http.run().await
            });
            self.handles.push(handle);
        }

        #[cfg(feature = "service")]
        if let Some(grpc) = self.grpc_server.take() {
            let handle = tokio::spawn(async move {
                grpc.run().await
            });
            self.handles.push(handle);
        }

        // Wait for all servers
        for handle in self.handles {
            if let Err(e) = handle.await? {
                error!("Server error: {:?}", e);
                return Err(e);
            }
        }

        Ok(())
    }
}

// Metrics endpoint for Prometheus
#[derive(Clone)]
pub struct MetricsService {
    registry: Arc<prometheus::Registry>,
}

impl MetricsService {
    pub fn new(registry: Arc<prometheus::Registry>) -> Self {
        Self { registry }
    }

    pub async fn serve(self, addr: SocketAddr) -> Result<(), Box<dyn std::error::Error + Send + Sync + 'static>> {
        use axum::response::Response;
        use axum::body::Body;
        use prometheus::{Encoder, TextEncoder};
        
        let app = Router::new()
            .route("/metrics", get(move || async move {
                let mut buffer = vec![];
                let encoder = TextEncoder::new();
                let metric_families = self.registry.gather();
                encoder.encode(&metric_families, &mut buffer).unwrap();
                
                Response::builder()
                    .header("Content-Type", encoder.format_type())
                    .body(Body::from(buffer))
                    .unwrap()
            }));

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

    pub async fn check(&self) -> Result<HealthResponse, Box<dyn std::error::Error + Send + Sync + 'static>> {
        // TODO: Implement actual health checks
        Ok(HealthResponse {
            status: "healthy".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            uptime_seconds: 0,
        })
    }
}

#[cfg(test)]
mod tests {
    
    #[tokio::test]
    async fn test_health_endpoint() {
        // TODO: Implement tests
    }
}