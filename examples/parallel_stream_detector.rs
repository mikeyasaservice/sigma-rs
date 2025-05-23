//! Parallel stream detector - multi-threaded Sigma rule processing for high throughput
//! 
//! This example reads JSON events from stdin, distributes them across multiple
//! worker threads for rule processing, and outputs matches with backpressure control.
//! 
//! Usage:
//!     cat events.json | cargo run --example parallel_stream_detector -- --rule-dirs /path/to/rules
//!     tail -f /var/log/events.json | cargo run --example parallel_stream_detector -- --rule-dirs ./rules --workers 8
//!     cat big_dataset.ndjson | cargo run --example parallel_stream_detector -- --rule-dirs ./rules --batch-size 50

use clap::Parser;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Instant, Duration};
use sigma_rs::{DynamicEvent, Event};
use sigma_rs::error::Result;
use sigma_rs::rule::Rule;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt};
use tokio::signal;
use tokio::sync::{mpsc, Semaphore};
use tracing::{info, warn, error};
use serde_json::Value;
use futures::stream::{Stream, StreamExt};

mod common;
use common::{
    CommonArgs, DetectionResult, RuleMatch, Event as EventStruct,
    find_rule_files, load_rules_with_progress, setup_logging, format_duration
};

// Use the new RuleSet from the library
use sigma_rs::RuleSet;
    
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
#[command(name = "parallel_stream_detector")]
#[command(about = "Multi-threaded Sigma rule processing for high throughput")]
#[command(version)]
struct Args {
    #[command(flatten)]
    common: CommonArgs,
    
    /// Number of worker threads
    #[arg(long, default_value_t = 4)]
    workers: usize,
    
    /// Size of event processing queue
    #[arg(long, default_value_t = 1000)]
    queue_size: usize,
    
    /// Batch size for event processing
    #[arg(long, default_value_t = 10)]
    batch_size: usize,
    
    /// Output format: json, csv, or compact
    #[arg(long, default_value = "json")]
    format: OutputFormat,
    
    /// Report metrics every N seconds
    #[arg(long, default_value_t = 10)]
    metrics_interval: u64,
    
    /// Maximum pending writes (backpressure)
    #[arg(long, default_value_t = 100)]
    max_pending_writes: usize,
    
    /// Continue on parsing errors
    #[arg(long)]
    continue_on_error: bool,
    
    /// Enable adaptive worker scaling
    #[arg(long)]
    adaptive_scaling: bool,
}

#[derive(Debug, Clone, Copy, clap::ValueEnum)]
enum OutputFormat {
    Json,
    Csv,
    Compact,
}

type EventBatch = Vec<Value>;

struct ProcessingMetrics {
    events_received: AtomicU64,
    events_processed: AtomicU64,
    events_matched: AtomicU64,
    events_failed: AtomicU64,
    batches_processed: AtomicU64,
    queue_depth: AtomicU64,
    start_time: Instant,
}

impl ProcessingMetrics {
    fn new() -> Self {
        Self {
            events_received: AtomicU64::new(0),
            events_processed: AtomicU64::new(0),
            events_matched: AtomicU64::new(0),
            events_failed: AtomicU64::new(0),
            batches_processed: AtomicU64::new(0),
            queue_depth: AtomicU64::new(0),
            start_time: Instant::now(),
        }
    }
    
    fn report(&self) {
        let received = self.events_received.load(Ordering::Relaxed);
        let processed = self.events_processed.load(Ordering::Relaxed);
        let matched = self.events_matched.load(Ordering::Relaxed);
        let failed = self.events_failed.load(Ordering::Relaxed);
        let batches = self.batches_processed.load(Ordering::Relaxed);
        let queue_depth = self.queue_depth.load(Ordering::Relaxed);
        
        let elapsed = self.start_time.elapsed();
        let rate = processed as f64 / elapsed.as_secs_f64();
        
        info!(
            "Metrics: received={}, processed={}, matched={}, failed={}, batches={}, queue={}, rate={:.1}/s",
            received, processed, matched, failed, batches, queue_depth, rate
        );
    }
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
    let mut ruleset = RuleSet::new();
    for rule in rules {
        ruleset.add_rule(rule).await.unwrap();
    }
    let ruleset = Arc::new(ruleset);
    info!("Created ruleset with {} active rules", ruleset.len());
    
    // Setup channels and metrics
    let (event_tx, event_rx) = mpsc::channel::<EventBatch>(args.queue_size);
    let (result_tx, result_rx) = mpsc::channel::<DetectionResult>(args.queue_size);
    let metrics = Arc::new(ProcessingMetrics::new());
    let shutdown = Arc::new(AtomicBool::new(false));
    let write_sem = Arc::new(Semaphore::new(args.max_pending_writes));
    
    // Start components
    let reader_handle = start_event_reader(event_tx, metrics.clone(), shutdown.clone(), &args);
    let processor_handles = start_processors(
        event_rx,
        result_tx,
        ruleset,
        metrics.clone(),
        shutdown.clone(),
        args.workers,
    );
    let writer_handle = start_result_writer(
        result_rx,
        write_sem.clone(),
        metrics.clone(),
        shutdown.clone(),
        args.format,
    );
    let metrics_handle = start_metrics_reporter(
        metrics.clone(),
        args.metrics_interval,
        shutdown.clone(),
    );
    
    // Handle shutdown
    tokio::select! {
        _ = signal::ctrl_c() => {
            info!("Received interrupt signal, shutting down...");
            shutdown.store(true, Ordering::Relaxed);
        }
        _ = reader_handle => {}
        _ = async {
            for handle in processor_handles {
                handle.await.ok();
            }
        } => {}
        _ = writer_handle => {}
        _ = metrics_handle => {}
    }
    
    // Final metrics
    metrics.report();
    info!("Parallel stream processing completed");
    
    Ok(())
}

async fn start_event_reader(
    tx: mpsc::Sender<EventBatch>,
    metrics: Arc<ProcessingMetrics>,
    shutdown: Arc<AtomicBool>,
    args: &Args,
) -> tokio::task::JoinHandle<()> {
    let batch_size = args.batch_size;
    let continue_on_error = args.continue_on_error;
    
    tokio::spawn(async move {
        let stdin = tokio::io::stdin();
        let mut reader = tokio::io::BufReader::new(stdin);
        let mut line = String::new();
        let mut batch = Vec::with_capacity(batch_size);
        
        while !shutdown.load(Ordering::Relaxed) {
            line.clear();
            match reader.read_line(&mut line).await {
                Ok(0) => break, // EOF
                Ok(_) => {
                    if line.trim().is_empty() {
                        continue;
                    }
                    
                    match serde_json::from_str::<Value>(&line) {
                        Ok(event) => {
                            metrics.events_received.fetch_add(1, Ordering::Relaxed);
                            batch.push(event);
                            
                            if batch.len() >= batch_size {
                                let current_batch = std::mem::replace(
                                    &mut batch,
                                    Vec::with_capacity(batch_size)
                                );
                                
                                metrics.queue_depth.store(
                                    tx.capacity() as u64,
                                    Ordering::Relaxed
                                );
                                
                                if tx.send(current_batch).await.is_err() {
                                    error!("Failed to send batch to processors");
                                    break;
                                }
                            }
                        }
                        Err(e) => {
                            metrics.events_failed.fetch_add(1, Ordering::Relaxed);
                            if continue_on_error {
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
        
        // Send final batch
        if !batch.is_empty() {
            tx.send(batch).await.ok();
        }
    })
}

fn start_processors(
    mut rx: mpsc::Receiver<EventBatch>,
    tx: mpsc::Sender<DetectionResult>,
    ruleset: Arc<RuleSet>,
    metrics: Arc<ProcessingMetrics>,
    shutdown: Arc<AtomicBool>,
    num_workers: usize,
) -> Vec<tokio::task::JoinHandle<()>> {
    let mut handles = Vec::new();
    
    for worker_id in 0..num_workers {
        let tx = tx.clone();
        let ruleset = ruleset.clone();
        let metrics = metrics.clone();
        let shutdown = shutdown.clone();
        
        let handle = tokio::spawn(async move {
            info!("Worker {} started", worker_id);
            
            while !shutdown.load(Ordering::Relaxed) {
                match rx.recv().await {
                    Some(batch) => {
                        for event_value in batch {
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
                                        severity: rule_match.severity.clone()
                                            .unwrap_or_else(|| "unknown".to_string()),
                                        tags: rule_match.tags.clone(),
                                        score: rule_match.score,
                                    });
                                }
                            }
                            
                            let processing_time = start.elapsed();
                            
                            if !matches.is_empty() {
                                let result = DetectionResult {
                                    event: EventStruct { 
                                        data: event_value.as_object().unwrap().clone() 
                                    },
                                    matches,
                                    timestamp: chrono::Utc::now(),
                                    processing_time_ms: processing_time.as_millis() as u64,
                                };
                                
                                metrics.events_matched.fetch_add(1, Ordering::Relaxed);
                                
                                if tx.send(result).await.is_err() {
                                    error!("Worker {}: Failed to send result", worker_id);
                                    break;
                                }
                            }
                            
                            metrics.events_processed.fetch_add(1, Ordering::Relaxed);
                        }
                        
                        metrics.batches_processed.fetch_add(1, Ordering::Relaxed);
                    }
                    None => break,
                }
            }
            
            info!("Worker {} stopped", worker_id);
        });
        
        handles.push(handle);
    }
    
    handles
}

async fn start_result_writer(
    mut rx: mpsc::Receiver<DetectionResult>,
    write_sem: Arc<Semaphore>,
    metrics: Arc<ProcessingMetrics>,
    shutdown: Arc<AtomicBool>,
    format: OutputFormat,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut stdout = tokio::io::stdout();
        
        // Write CSV header if needed
        if matches!(format, OutputFormat::Csv) {
            let header = "timestamp,event_id,rule_id,rule_name,severity,tags\n";
            stdout.write_all(header.as_bytes()).await.unwrap();
        }
        
        while !shutdown.load(Ordering::Relaxed) {
            match rx.recv().await {
                Some(result) => {
                    // Acquire semaphore permit for backpressure
                    let _permit = write_sem.acquire().await.unwrap();
                    
                    match format {
                        OutputFormat::Json => {
                            let json = serde_json::to_string(&result).unwrap();
                            stdout.write_all(json.as_bytes()).await.unwrap();
                            stdout.write_all(b"\n").await.unwrap();
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
                                stdout.write_all(line.as_bytes()).await.unwrap();
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
                            stdout.write_all(line.as_bytes()).await.unwrap();
                        }
                    }
                    
                    stdout.flush().await.unwrap();
                }
                None => break,
            }
        }
    })
}

async fn start_metrics_reporter(
    metrics: Arc<ProcessingMetrics>,
    interval_secs: u64,
    shutdown: Arc<AtomicBool>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(interval_secs));
        
        while !shutdown.load(Ordering::Relaxed) {
            interval.tick().await;
            metrics.report();
        }
    })
}