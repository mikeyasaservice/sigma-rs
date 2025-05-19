//! Stream detector - applies Sigma rules to streaming JSON events from stdin
//! 
//! This example reads JSON events from stdin, applies Sigma rules, and outputs
//! matches to stdout. Supports graceful shutdown and metrics collection.
//! 
//! Usage:
//!     cat events.json | cargo run --example stream_detector -- --rule-dirs /path/to/rules
//!     tail -f /var/log/events.json | cargo run --example stream_detector -- --rule-dirs ./rules
//!     cat events.ndjson | cargo run --example stream_detector -- --rule-dirs ./rules --format csv

use clap::Parser;
use std::path::PathBuf;
use std::io::{stdin, stdout, BufReader, Write};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Instant, Duration};
use sigma_rs::{DynamicEvent, Event};
use sigma_rs::rule::Rule;
use sigma_rs::error::Result;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt};
use tokio::signal;
use tokio::sync::Mutex;
use tracing::{info, warn, error};
use serde_json::{Deserializer, Value};

mod common;
use common::{
    CommonArgs, DetectionResult, RuleMatch, Event as EventStruct,
    find_rule_files, load_rules_with_progress, setup_logging, format_duration
};

// Placeholder RuleSet since it's not available in the public API
struct RuleSet {
    rules: Vec<Rule>,
}

impl RuleSet {
    fn new(rules: Vec<Rule>) -> Self {
        Self { rules }
    }
    
    fn len(&self) -> usize {
        self.rules.len()
    }
    
    // Placeholder method - actual implementation would evaluate rules
    fn eval_all(&self, _event: &DynamicEvent) -> Vec<(String, Vec<MockMatch>)> {
        Vec::new()
    }
}

// Mock match result
struct MockMatch {
    rule_id: String,
    rule_path: Option<String>,
    severity: Option<String>,
    tags: Vec<String>,
    score: f64,
}

#[derive(Parser, Debug)]
#[command(name = "stream_detector")]
#[command(about = "Applies Sigma rules to streaming JSON events from stdin")]
#[command(version)]
struct Args {
    #[command(flatten)]
    common: CommonArgs,
    
    /// Output format: json, csv, or compact
    #[arg(long, default_value = "json")]
    format: OutputFormat,
    
    /// Report metrics every N seconds
    #[arg(long, default_value_t = 10)]
    metrics_interval: u64,
    
    /// Buffer size for event processing
    #[arg(long, default_value_t = 100)]
    buffer_size: usize,
    
    /// Continue on parsing errors
    #[arg(long)]
    continue_on_error: bool,
}

#[derive(Debug, Clone, Copy, clap::ValueEnum)]
enum OutputFormat {
    Json,
    Csv,
    Compact,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    setup_logging(args.common.verbose);
    
    if args.common.rule_dirs.is_empty() {
        error!("No rule directories specified");
        std::process::exit(1);
    }
    
    // Load rules
    let files = find_rule_files(&args.common.rule_dirs)?;
    if files.is_empty() {
        error!("No rule files found");
        std::process::exit(1);
    }
    
    let (rules, stats) = load_rules_with_progress(files)?;
    info!("Loaded {} rules (failed: {}, unsupported: {})", 
        stats.parsed, stats.failed, stats.unsupported);
    
    // Create ruleset
    let ruleset = Arc::new(RuleSet::new(rules));
    info!("Created ruleset with {} active rules", ruleset.len());
    
    // Setup metrics
    let metrics = Arc::new(StreamMetrics::new());
    let shutdown = Arc::new(AtomicBool::new(false));
    
    // Start metrics reporter
    let metrics_handle = start_metrics_reporter(
        metrics.clone(),
        args.metrics_interval,
        shutdown.clone()
    );
    
    // Start event processor
    let processor_handle = start_event_processor(
        ruleset,
        metrics.clone(),
        shutdown.clone(),
        args
    );
    
    // Setup signal handlers
    tokio::select! {
        _ = signal::ctrl_c() => {
            info!("Received interrupt signal, shutting down...");
            shutdown.store(true, Ordering::Relaxed);
        }
        _ = processor_handle => {}
        _ = metrics_handle => {}
    }
    
    // Final metrics
    metrics.report();
    info!("Stream processing completed");
    
    Ok(())
}

struct StreamMetrics {
    events_processed: AtomicU64,
    events_matched: AtomicU64,
    events_failed: AtomicU64,
    start_time: Instant,
    last_report: Mutex<Instant>,
}

impl StreamMetrics {
    fn new() -> Self {
        Self {
            events_processed: AtomicU64::new(0),
            events_matched: AtomicU64::new(0),
            events_failed: AtomicU64::new(0),
            start_time: Instant::now(),
            last_report: Mutex::new(Instant::now()),
        }
    }
    
    async fn report(&self) {
        let processed = self.events_processed.load(Ordering::Relaxed);
        let matched = self.events_matched.load(Ordering::Relaxed);
        let failed = self.events_failed.load(Ordering::Relaxed);
        
        let elapsed = self.start_time.elapsed();
        let mut last_report = self.last_report.lock().await;
        let period = last_report.elapsed();
        *last_report = Instant::now();
        
        let rate = processed as f64 / elapsed.as_secs_f64();
        let period_rate = processed as f64 / period.as_secs_f64();
        
        info!(
            "Metrics: processed={}, matched={}, failed={}, rate={:.1}/s, period_rate={:.1}/s",
            processed, matched, failed, rate, period_rate
        );
    }
}

async fn start_metrics_reporter(
    metrics: Arc<StreamMetrics>,
    interval_secs: u64,
    shutdown: Arc<AtomicBool>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(interval_secs));
        
        while !shutdown.load(Ordering::Relaxed) {
            interval.tick().await;
            metrics.report().await;
        }
    })
}

async fn start_event_processor(
    ruleset: Arc<RuleSet>,
    metrics: Arc<StreamMetrics>,
    shutdown: Arc<AtomicBool>,
    args: Args,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let stdin = tokio::io::stdin();
        let mut reader = tokio::io::BufReader::new(stdin);
        let mut stdout = tokio::io::stdout();
        let mut line = String::new();
        
        // Write CSV header if needed
        if matches!(args.format, OutputFormat::Csv) {
            let header = "timestamp,event_id,rule_id,rule_name,severity,tags\n";
            stdout.write_all(header.as_bytes()).await.unwrap();
        }
        
        loop {
            if shutdown.load(Ordering::Relaxed) {
                break;
            }
            
            line.clear();
            match reader.read_line(&mut line).await {
                Ok(0) => break, // EOF
                Ok(_) => {
                    if line.trim().is_empty() {
                        continue;
                    }
                    
                    match serde_json::from_str::<Value>(&line) {
                        Ok(event_value) => {
                            let result = process_event(event_value, &ruleset).await;
                            metrics.events_processed.fetch_add(1, Ordering::Relaxed);
                            
                            if !result.matches.is_empty() {
                                metrics.events_matched.fetch_add(1, Ordering::Relaxed);
                                
                                match write_result(&mut stdout, &result, args.format).await {
                                    Ok(_) => {}
                                    Err(e) => {
                                        error!("Failed to write result: {}", e);
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            metrics.events_failed.fetch_add(1, Ordering::Relaxed);
                            if args.continue_on_error {
                                warn!("Failed to parse event: {}", e);
                            } else {
                                error!("Failed to parse event: {}", e);
                                break;
                            }
                        }
                    }
                }
                Err(e) => {
                    error!("Failed to read from stdin: {}", e);
                    break;
                }
            }
        }
    })
}

async fn process_event(event_value: Value, ruleset: &RuleSet) -> DetectionResult {
    let start = Instant::now();
    let event = DynamicEvent::new(event_value.clone());
    
    let mut matches = Vec::new();
    
    // Apply all rules
    for (rule_name, rule_matches) in ruleset.eval_all(&event) {
        for rule_match in rule_matches {
            matches.push(RuleMatch {
                rule_id: rule_match.rule_id.clone(),
                rule_name: rule_name.clone(),
                rule_path: rule_match.rule_path.clone().unwrap_or_default(),
                severity: rule_match.severity.clone().unwrap_or_else(|| "unknown".to_string()),
                tags: rule_match.tags.clone(),
                score: rule_match.score,
            });
        }
    }
    
    let processing_time = start.elapsed();
    
    DetectionResult {
        event: EventStruct { data: event_value.as_object().unwrap().clone() },
        matches,
        timestamp: chrono::Utc::now(),
        processing_time_ms: processing_time.as_millis() as u64,
    }
}

async fn write_result(
    writer: &mut tokio::io::Stdout,
    result: &DetectionResult,
    format: OutputFormat,
) -> Result<()> {
    match format {
        OutputFormat::Json => {
            let json = serde_json::to_string(result)?;
            writer.write_all(json.as_bytes()).await?;
            writer.write_all(b"\n").await?;
        }
        OutputFormat::Csv => {
            for rule_match in &result.matches {
                let line = format!(
                    "{},{},{},{},{},{}\n",
                    result.timestamp.to_rfc3339(),
                    result.event.data.get("EventRecordID")
                        .and_then(|v| v.as_str())
                        .unwrap_or("unknown"),
                    rule_match.rule_id,
                    rule_match.rule_name,
                    rule_match.severity,
                    rule_match.tags.join(";")
                );
                writer.write_all(line.as_bytes()).await?;
            }
        }
        OutputFormat::Compact => {
            let line = format!(
                "[{}] {} matches: {}\n",
                result.timestamp.to_rfc3339(),
                result.matches.len(),
                result.matches.iter()
                    .map(|m| &m.rule_name)
                    .collect::<Vec<_>>()
                    .join(", ")
            );
            writer.write_all(line.as_bytes()).await?;
        }
    }
    
    writer.flush().await?;
    Ok(())
}