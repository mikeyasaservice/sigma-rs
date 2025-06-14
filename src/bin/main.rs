//! Command-line interface for sigma-rs
//!
//! This CLI provides a production-ready interface for processing security events
//! using Sigma rules with support for various input sources and output formats.

use anyhow::{Context, Result};
use clap::{Parser, Subcommand, ValueEnum};
use indicatif::{ProgressBar, ProgressStyle};
use serde_json::Value;
use sigma_rs::{
    init_tracing, DynamicEvent, RuleSet, RuleSetResult, SigmaEngine, SigmaEngineBuilder,
};
use std::io::{self, BufRead, BufReader, Write};
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::signal;
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};

#[cfg(feature = "kafka")]
use sigma_rs::consumer::{create_sigma_consumer, ConsumerConfig, MessageProcessor};
#[cfg(feature = "kafka")]
use sigma_rs::KafkaConfig;

/// Sigma rule engine for security event detection
#[derive(Parser, Debug)]
#[command(name = "sigma-rs")]
#[command(version = sigma_rs::VERSION)]
#[command(about = "High-performance Sigma rule engine for security event detection", long_about = None)]
struct Cli {
    /// Directories containing Sigma rules (YAML files)
    #[arg(short, long, value_name = "DIR", num_args = 1.., required = true)]
    rules: Vec<String>,

    /// Input source for events
    #[command(subcommand)]
    source: Option<EventSource>,

    /// Output format for matches
    #[arg(short, long, value_enum, default_value = "human")]
    output: OutputFormat,

    /// Show detailed information about rule evaluation
    #[arg(short, long)]
    verbose: bool,

    /// Fail if any rule fails to parse
    #[arg(long)]
    fail_on_parse_error: bool,

    /// Number of worker threads (defaults to number of CPU cores)
    #[arg(short = 'j', long)]
    workers: Option<usize>,

    /// Show progress bar for batch processing
    #[arg(long)]
    progress: bool,

    /// Only show events that match at least one rule
    #[arg(long)]
    matched_only: bool,

    /// Write output to file instead of stdout
    #[arg(short = 'o', long)]
    output_file: Option<String>,

    /// Enable JSON structured logging
    #[arg(long)]
    json_logs: bool,

    /// Log level (trace, debug, info, warn, error)
    #[arg(long, default_value = "info")]
    log_level: String,
}

#[derive(Subcommand, Debug)]
enum EventSource {
    /// Process events from a file
    File {
        /// Path to the events file (JSON or JSONL format)
        #[arg(value_name = "FILE")]
        path: String,
    },
    /// Process events from stdin (default)
    Stdin,
    /// Stream events from Kafka/Redpanda
    #[cfg(feature = "kafka")]
    Kafka {
        /// Kafka broker addresses
        #[arg(long, default_value = "localhost:9092")]
        brokers: String,
        /// Consumer group ID
        #[arg(long, default_value = "sigma-cli")]
        group_id: String,
        /// Topics to consume from
        #[arg(long, num_args = 1.., required = true)]
        topics: Vec<String>,
        /// Dead letter queue topic for failed messages
        #[arg(long)]
        dlq_topic: Option<String>,
        /// Maximum number of retries for failed messages
        #[arg(long, default_value = "3")]
        max_retries: u32,
        /// Batch size for processing
        #[arg(long, default_value = "1000")]
        batch_size: usize,
    },
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum OutputFormat {
    /// Human-readable output
    Human,
    /// JSON output (one object per event)
    Json,
    /// CSV output
    Csv,
    /// Compact output (just rule IDs)
    Compact,
}

/// Statistics for the processing session
struct ProcessingStats {
    events_processed: AtomicU64,
    events_matched: AtomicU64,
    events_failed: AtomicU64,
    start_time: Instant,
}

impl ProcessingStats {
    fn new() -> Self {
        Self {
            events_processed: AtomicU64::new(0),
            events_matched: AtomicU64::new(0),
            events_failed: AtomicU64::new(0),
            start_time: Instant::now(),
        }
    }

    fn increment_processed(&self) {
        self.events_processed.fetch_add(1, Ordering::Relaxed);
    }

    fn increment_matched(&self) {
        self.events_matched.fetch_add(1, Ordering::Relaxed);
    }

    fn increment_failed(&self) {
        self.events_failed.fetch_add(1, Ordering::Relaxed);
    }

    fn summary(&self) -> String {
        let duration = self.start_time.elapsed();
        let processed = self.events_processed.load(Ordering::Relaxed);
        let matched = self.events_matched.load(Ordering::Relaxed);
        let failed = self.events_failed.load(Ordering::Relaxed);
        let rate = if duration.as_secs() > 0 {
            processed as f64 / duration.as_secs_f64()
        } else {
            0.0
        };

        format!(
            "Processed {} events in {:?} ({:.2} events/sec): {} matched, {} failed",
            processed, duration, rate, matched, failed
        )
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Initialize logging
    if cli.json_logs {
        init_tracing();
    } else {
        init_custom_tracing(&cli.log_level);
    }

    // Set up graceful shutdown
    let shutdown = Arc::new(AtomicBool::new(false));
    let shutdown_clone = shutdown.clone();
    
    tokio::spawn(async move {
        match signal::ctrl_c().await {
            Ok(()) => {
                info!("Received shutdown signal");
                shutdown_clone.store(true, Ordering::Relaxed);
            }
            Err(err) => {
                error!("Failed to listen for shutdown signal: {}", err);
            }
        }
    });

    // Build the Sigma engine
    let mut builder = SigmaEngineBuilder::new()
        .fail_on_parse_error(cli.fail_on_parse_error)
        .collapse_whitespace(true);

    if let Some(workers) = cli.workers {
        builder = builder.worker_threads(workers);
    }

    for dir in &cli.rules {
        builder = builder.add_rule_dir(dir);
    }

    info!("Loading rules from {} directories", cli.rules.len());
    let engine = Arc::new(SigmaEngine::new(builder).await.context("Failed to create Sigma engine")?);
    
    let rule_count = engine.ruleset().len();
    info!("Loaded {} rules successfully", rule_count);

    if rule_count == 0 {
        warn!("No rules loaded. Please check your rule directories.");
        return Ok(());
    }

    // Set up output writer
    let output_writer: Box<dyn Write + Send> = if let Some(output_file) = cli.output_file {
        Box::new(std::fs::File::create(&output_file).context("Failed to create output file")?)
    } else {
        Box::new(io::stdout())
    };
    let output_writer = Arc::new(tokio::sync::Mutex::new(output_writer));

    // Write CSV header if needed
    if matches!(cli.output, OutputFormat::Csv) {
        let mut writer = output_writer.lock().await;
        writeln!(writer, "timestamp,event_id,rule_id,rule_title,matched")?;
    }

    // Process events based on source
    let stats = Arc::new(ProcessingStats::new());
    
    match cli.source.unwrap_or(EventSource::Stdin) {
        EventSource::File { path } => {
            process_file(&path, engine, &cli, output_writer, stats, shutdown).await?;
        }
        EventSource::Stdin => {
            process_stdin(engine, &cli, output_writer, stats, shutdown).await?;
        }
        #[cfg(feature = "kafka")]
        EventSource::Kafka {
            brokers,
            group_id,
            topics,
            dlq_topic,
            max_retries,
            batch_size,
        } => {
            process_kafka(
                engine,
                &cli,
                output_writer,
                stats,
                shutdown,
                KafkaConfig {
                    brokers,
                    group_id,
                    topics,
                    dlq_topic,
                    max_retries: Some(max_retries),
                    batch_size: Some(batch_size),
                    ..Default::default()
                },
            )
            .await?;
        }
    }

    // Print final statistics
    info!("{}", stats.summary());
    
    Ok(())
}

/// Process events from a file
async fn process_file(
    path: &str,
    engine: Arc<SigmaEngine>,
    cli: &Cli,
    output_writer: Arc<tokio::sync::Mutex<Box<dyn Write + Send>>>,
    stats: Arc<ProcessingStats>,
    shutdown: Arc<AtomicBool>,
) -> Result<()> {
    let file = std::fs::File::open(path).context("Failed to open input file")?;
    let reader = BufReader::new(file);
    
    // Count lines for progress bar
    let line_count = if cli.progress {
        let file = std::fs::File::open(path)?;
        BufReader::new(file).lines().count() as u64
    } else {
        0
    };

    let progress = if cli.progress && line_count > 0 {
        let pb = ProgressBar::new(line_count);
        pb.set_style(
            ProgressStyle::default_bar()
                .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({eta})")
                .unwrap()
                .progress_chars("#>-"),
        );
        Some(pb)
    } else {
        None
    };

    let file = std::fs::File::open(path)?;
    let reader = BufReader::new(file);

    for (line_num, line) in reader.lines().enumerate() {
        if shutdown.load(Ordering::Relaxed) {
            info!("Shutting down...");
            break;
        }

        if let Some(pb) = &progress {
            pb.inc(1);
        }

        let line = line.context("Failed to read line from file")?;
        if line.trim().is_empty() {
            continue;
        }

        match serde_json::from_str::<Value>(&line) {
            Ok(json) => {
                let event = DynamicEvent::new(json);
                process_event(
                    event,
                    &engine,
                    cli,
                    &output_writer,
                    &stats,
                    line_num + 1,
                )
                .await?;
            }
            Err(e) => {
                error!("Failed to parse JSON at line {}: {}", line_num + 1, e);
                stats.increment_failed();
            }
        }
    }

    if let Some(pb) = progress {
        pb.finish_with_message("Processing complete");
    }

    Ok(())
}

/// Process events from stdin
async fn process_stdin(
    engine: Arc<SigmaEngine>,
    cli: &Cli,
    output_writer: Arc<tokio::sync::Mutex<Box<dyn Write + Send>>>,
    stats: Arc<ProcessingStats>,
    shutdown: Arc<AtomicBool>,
) -> Result<()> {
    let stdin = io::stdin();
    let reader = stdin.lock();

    for (line_num, line) in reader.lines().enumerate() {
        if shutdown.load(Ordering::Relaxed) {
            info!("Shutting down...");
            break;
        }

        let line = line.context("Failed to read line from stdin")?;
        if line.trim().is_empty() {
            continue;
        }

        match serde_json::from_str::<Value>(&line) {
            Ok(json) => {
                let event = DynamicEvent::new(json);
                process_event(
                    event,
                    &engine,
                    cli,
                    &output_writer,
                    &stats,
                    line_num + 1,
                )
                .await?;
            }
            Err(e) => {
                error!("Failed to parse JSON at line {}: {}", line_num + 1, e);
                stats.increment_failed();
            }
        }
    }

    Ok(())
}

/// Process events from Kafka
#[cfg(feature = "kafka")]
async fn process_kafka(
    engine: Arc<SigmaEngine>,
    cli: &Cli,
    output_writer: Arc<tokio::sync::Mutex<Box<dyn Write + Send>>>,
    stats: Arc<ProcessingStats>,
    shutdown: Arc<AtomicBool>,
    kafka_config: KafkaConfig,
) -> Result<()> {
    info!("Starting Kafka consumer: brokers={}, topics={:?}", 
        kafka_config.brokers, kafka_config.topics);

    // Create a channel for events
    let (tx, mut rx) = mpsc::channel::<DynamicEvent>(1000);

    // Create message processor
    let processor = CliMessageProcessor {
        engine: engine.clone(),
        cli_config: CliProcessorConfig {
            output_format: cli.output,
            verbose: cli.verbose,
            matched_only: cli.matched_only,
        },
        output_writer: output_writer.clone(),
        stats: stats.clone(),
        tx,
    };

    // Create consumer config
    let consumer_config = ConsumerConfig::builder()
        .brokers(kafka_config.brokers)
        .group_id(kafka_config.group_id)
        .topics(kafka_config.topics)
        .dlq_topic(kafka_config.dlq_topic)
        .max_retries(kafka_config.max_retries.unwrap_or(3))
        .batch_size(kafka_config.batch_size.unwrap_or(1000))
        .build();

    // Create and run consumer
    let consumer = create_sigma_consumer(Arc::new(processor), consumer_config).await?;
    
    // Run consumer in background
    let consumer_handle = tokio::spawn(async move {
        consumer.run().await
    });

    // Process events from channel
    let mut event_num = 0;
    while !shutdown.load(Ordering::Relaxed) {
        match rx.recv().await {
            Some(event) => {
                event_num += 1;
                process_event(
                    event,
                    &engine,
                    cli,
                    &output_writer,
                    &stats,
                    event_num,
                )
                .await?;
            }
            None => {
                break;
            }
        }
    }

    consumer_handle.abort();
    Ok(())
}

/// Process a single event
async fn process_event(
    event: DynamicEvent,
    engine: &Arc<SigmaEngine>,
    cli: &Cli,
    output_writer: &Arc<tokio::sync::Mutex<Box<dyn Write + Send>>>,
    stats: &Arc<ProcessingStats>,
    event_num: usize,
) -> Result<()> {
    stats.increment_processed();

    match engine.process_event(event.clone()).await {
        Ok(result) => {
            let has_match = result.matches.iter().any(|m| m.matched);
            
            if has_match {
                stats.increment_matched();
            }

            if !cli.matched_only || has_match {
                output_result(
                    &event,
                    &result,
                    cli.output,
                    output_writer,
                    event_num,
                    cli.verbose,
                )
                .await?;
            }
        }
        Err(e) => {
            error!("Failed to process event {}: {}", event_num, e);
            stats.increment_failed();
        }
    }

    Ok(())
}

/// Output the result in the specified format
async fn output_result(
    event: &DynamicEvent,
    result: &RuleSetResult,
    format: OutputFormat,
    output_writer: &Arc<tokio::sync::Mutex<Box<dyn Write + Send>>>,
    event_num: usize,
    verbose: bool,
) -> Result<()> {
    let mut writer = output_writer.lock().await;
    
    match format {
        OutputFormat::Human => {
            write_human_output(&mut **writer, event, result, event_num, verbose)?;
        }
        OutputFormat::Json => {
            write_json_output(&mut **writer, event, result, event_num)?;
        }
        OutputFormat::Csv => {
            write_csv_output(&mut **writer, event, result, event_num)?;
        }
        OutputFormat::Compact => {
            write_compact_output(&mut **writer, result)?;
        }
    }

    Ok(())
}

/// Write human-readable output
fn write_human_output(
    writer: &mut dyn Write,
    event: &DynamicEvent,
    result: &RuleSetResult,
    event_num: usize,
    verbose: bool,
) -> Result<()> {
    let matches: Vec<_> = result.matches.iter().filter(|m| m.matched).collect();
    
    if !matches.is_empty() {
        writeln!(writer, "\n[Event #{}] ✓ {} rule(s) matched", event_num, matches.len())?;
        
        for m in &matches {
            writeln!(writer, "  • {} - {}", m.rule_id, m.rule_title)?;
            if verbose {
                writeln!(writer, "    Evaluation time: {:?}", m.evaluation_time)?;
            }
        }
        
        if verbose {
            writeln!(writer, "  Event: {}", serde_json::to_string(&event.data).unwrap_or_default())?;
            writeln!(writer, "  Total evaluation time: {:?}", result.evaluation_time)?;
        }
    } else if verbose {
        writeln!(writer, "\n[Event #{}] ✗ No rules matched", event_num)?;
        writeln!(writer, "  Evaluated {} rules in {:?}", result.rules_evaluated, result.evaluation_time)?;
    }
    
    Ok(())
}

/// Write JSON output
fn write_json_output(
    writer: &mut dyn Write,
    event: &DynamicEvent,
    result: &RuleSetResult,
    event_num: usize,
) -> Result<()> {
    let output = serde_json::json!({
        "event_num": event_num,
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "event": event.data,
        "matches": result.matches.iter()
            .filter(|m| m.matched)
            .map(|m| serde_json::json!({
                "rule_id": m.rule_id,
                "rule_title": m.rule_title,
                "evaluation_time_ms": m.evaluation_time.as_millis()
            }))
            .collect::<Vec<_>>(),
        "rules_evaluated": result.rules_evaluated,
        "total_evaluation_time_ms": result.evaluation_time.as_millis()
    });
    
    writeln!(writer, "{}", serde_json::to_string(&output)?)?;
    Ok(())
}

/// Write CSV output
fn write_csv_output(
    writer: &mut dyn Write,
    event: &DynamicEvent,
    result: &RuleSetResult,
    event_num: usize,
) -> Result<()> {
    let timestamp = chrono::Utc::now().to_rfc3339();
    
    for m in &result.matches {
        if m.matched {
            writeln!(
                writer,
                "{},{},{},{},true",
                timestamp,
                event_num,
                m.rule_id,
                m.rule_title.replace(',', ";") // Escape commas in title
            )?;
        }
    }
    
    Ok(())
}

/// Write compact output
fn write_compact_output(
    writer: &mut dyn Write,
    result: &RuleSetResult,
) -> Result<()> {
    let matched_ids: Vec<&str> = result.matches
        .iter()
        .filter(|m| m.matched)
        .map(|m| m.rule_id.as_str())
        .collect();
    
    if !matched_ids.is_empty() {
        writeln!(writer, "{}", matched_ids.join(","))?;
    }
    
    Ok(())
}

/// Custom tracing initialization for non-JSON output
fn init_custom_tracing(level: &str) {
    use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
    
    let env_filter = tracing_subscriber::EnvFilter::try_new(level)
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));
    
    tracing_subscriber::registry()
        .with(env_filter)
        .with(tracing_subscriber::fmt::layer().with_target(false))
        .init();
}

/// CLI-specific message processor for Kafka
#[cfg(feature = "kafka")]
struct CliMessageProcessor {
    engine: Arc<SigmaEngine>,
    cli_config: CliProcessorConfig,
    output_writer: Arc<tokio::sync::Mutex<Box<dyn Write + Send>>>,
    stats: Arc<ProcessingStats>,
    tx: mpsc::Sender<DynamicEvent>,
}

#[cfg(feature = "kafka")]
#[derive(Clone)]
struct CliProcessorConfig {
    output_format: OutputFormat,
    verbose: bool,
    matched_only: bool,
}

#[cfg(feature = "kafka")]
#[async_trait::async_trait]
impl MessageProcessor for CliMessageProcessor {
    async fn process_message(&self, event: DynamicEvent) -> Result<()> {
        // Send event through channel for processing
        self.tx.send(event).await.map_err(|e| anyhow::anyhow!("Failed to send event: {}", e))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cli_parsing() {
        let cli = Cli::try_parse_from(&[
            "sigma-rs",
            "--rules", "/path/to/rules",
            "--output", "json",
            "--verbose",
        ]);
        assert!(cli.is_ok());
    }

    #[test]
    fn test_stats() {
        let stats = ProcessingStats::new();
        stats.increment_processed();
        stats.increment_matched();
        assert_eq!(stats.events_processed.load(Ordering::Relaxed), 1);
        assert_eq!(stats.events_matched.load(Ordering::Relaxed), 1);
    }
}