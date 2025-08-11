//! HTTP/gRPC service binary for sigma-rs
//!
//! This binary starts the sigma-rs detection engine as a service with
//! REST and gRPC APIs for event evaluation.

use clap::Parser;
use sigma_rs::{SigmaEngine, SigmaEngineBuilder};
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use tracing::{error, info};
use tracing_subscriber;

#[cfg(feature = "service")]
use sigma_rs::service::{ServiceRunner, SigmaService};

#[derive(Parser)]
#[command(name = "sigma-rs-service")]
#[command(about = "Sigma-rs detection engine service", long_about = None)]
struct Args {
    /// Path to rules directory
    #[arg(short, long)]
    rules: Option<PathBuf>,

    /// HTTP service port
    #[arg(long, default_value = "8080")]
    http_port: u16,

    /// gRPC service port (if enabled)
    #[arg(long, default_value = "9090")]
    grpc_port: u16,

    /// Metrics port for Prometheus
    #[arg(long, default_value = "9091")]
    metrics_port: u16,

    /// Configuration file path
    #[arg(short, long)]
    config: Option<PathBuf>,

    /// Enable debug logging
    #[arg(short, long)]
    debug: bool,

    /// Disable HTTP service
    #[arg(long)]
    no_http: bool,

    /// Disable gRPC service
    #[arg(long)]
    no_grpc: bool,

    /// Enable metrics endpoint
    #[arg(long)]
    metrics: bool,
}

#[derive(Debug, serde::Deserialize)]
struct Config {
    #[serde(default)]
    service: ServiceConfig,
    #[serde(default)]
    engine: EngineConfig,
}

#[derive(Debug, serde::Deserialize)]
struct ServiceConfig {
    #[serde(default = "default_http_port")]
    http_port: u16,
    #[serde(default = "default_grpc_port")]
    grpc_port: u16,
    #[serde(default = "default_metrics_port")]
    metrics_port: u16,
}

#[derive(Debug, serde::Deserialize)]
struct EngineConfig {
    #[serde(default)]
    rules_dir: Option<String>,
    #[serde(default = "default_max_evaluations")]
    max_concurrent_evaluations: usize,
}

impl Default for ServiceConfig {
    fn default() -> Self {
        Self {
            http_port: default_http_port(),
            grpc_port: default_grpc_port(),
            metrics_port: default_metrics_port(),
        }
    }
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self {
            rules_dir: None,
            max_concurrent_evaluations: default_max_evaluations(),
        }
    }
}

fn default_http_port() -> u16 {
    8080
}
fn default_grpc_port() -> u16 {
    9090
}
fn default_metrics_port() -> u16 {
    9091
}
fn default_max_evaluations() -> usize {
    100
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Parse command line arguments
    let args = Args::parse();

    // Setup logging
    if args.debug {
        tracing_subscriber::fmt()
            .with_max_level(tracing::Level::DEBUG)
            .init();
    } else {
        tracing_subscriber::fmt()
            .with_max_level(tracing::Level::INFO)
            .init();
    }

    info!("Starting sigma-rs service");

    // Load configuration if provided
    let (http_port, grpc_port, _metrics_port, rules_dir) = if let Some(config_path) = &args.config {
        info!("Loading configuration from: {}", config_path.display());
        let config_str = std::fs::read_to_string(config_path)?;
        let config: Config = toml::from_str(&config_str)?;

        let rules = config
            .engine
            .rules_dir
            .map(PathBuf::from)
            .or_else(|| args.rules.clone())
            .expect("Rules directory must be specified via --rules or in config file");

        (
            config.service.http_port,
            config.service.grpc_port,
            config.service.metrics_port,
            rules,
        )
    } else {
        let rules = args
            .rules
            .clone()
            .expect("Rules directory must be specified via --rules");
        (args.http_port, args.grpc_port, args.metrics_port, rules)
    };

    // Validate rules directory
    if !rules_dir.exists() {
        error!("Rules directory not found: {}", rules_dir.display());
        std::process::exit(1);
    }

    if !rules_dir.is_dir() {
        error!("Rules path is not a directory: {}", rules_dir.display());
        std::process::exit(1);
    }

    info!("Loading rules from: {}", rules_dir.display());

    // Build the Sigma engine
    let engine = SigmaEngineBuilder::new()
        .add_rule_dir(rules_dir.to_string_lossy())
        .fail_on_parse_error(false)
        .build()
        .await?;

    let rule_count = engine.ruleset().len();

    if rule_count == 0 {
        error!("No rules loaded from directory: {}", rules_dir.display());
        std::process::exit(1);
    }

    info!("Loaded {} rules", rule_count);

    let engine = Arc::new(engine);

    // Check if service feature is enabled
    #[cfg(not(feature = "service"))]
    {
        error!("Service feature not enabled. Build with --features service");
        std::process::exit(1);
    }

    #[cfg(feature = "service")]
    {
        // Create service runner
        let mut runner = ServiceRunner::new();

        // Add HTTP service unless disabled
        if !args.no_http {
            let http_addr: SocketAddr = ([0, 0, 0, 0], http_port).into();
            info!("Starting HTTP service on {}", http_addr);
            runner = runner.with_http(Arc::clone(&engine), http_addr);
        }

        // Add gRPC service unless disabled
        #[cfg(feature = "service")]
        if !args.no_grpc {
            let grpc_addr: SocketAddr = ([0, 0, 0, 0], grpc_port).into();
            info!("Starting gRPC service on {}", grpc_addr);
            runner = runner.with_grpc(Arc::clone(&engine), grpc_addr);
        }

        // Set up graceful shutdown
        let shutdown = tokio::signal::ctrl_c();

        info!("Service ready");
        info!("  HTTP: http://localhost:{}/health", http_port);

        if !args.no_grpc {
            info!("  gRPC: localhost:{}", grpc_port);
        }

        if let Ok(api_key) = std::env::var("SIGMA_API_KEY") {
            if !api_key.is_empty() {
                info!("  API Key authentication enabled");
            }
        } else {
            info!("  API Key authentication disabled (set SIGMA_API_KEY to enable)");
        }

        info!("Press Ctrl+C to shutdown");

        // Run the service or wait for shutdown
        tokio::select! {
            result = runner.run() => {
                if let Err(e) = result {
                    error!("Service error: {}", e);
                    std::process::exit(1);
                }
            }
            _ = shutdown => {
                info!("Shutdown signal received");
            }
        }

        info!("Service stopped");
    }

    Ok(())
}
