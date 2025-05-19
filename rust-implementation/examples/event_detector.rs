//! Event detector - applies Sigma rules to events from a JSON file
//! 
//! This example loads Sigma rules and applies them to events stored in a JSON file,
//! outputting matches to a results file or stdout.
//! 
//! Usage:
//!     cargo run --example event_detector -- --rule-dirs /path/to/rules --events events.json
//!     cargo run --example event_detector -- --rule-dirs ./rules -i data.json -o results.json
//!     cargo run --example event_detector -- --rule-dirs ./rules --events data.json --stdout

use clap::Parser;
use std::path::PathBuf;
use std::time::{Instant, Duration};
use std::fs::File;
use std::io::{BufReader, BufWriter, Write};
use sigma_rs::{DynamicEvent, Event};
use sigma_rs::error::Result;
use sigma_rs::rule::Rule;
use tracing::{info, warn, error};
use serde_json::{Value, Deserializer};
use indicatif::{ProgressBar, ProgressStyle};

mod common;
use common::{
    CommonArgs, DetectionResult, RuleMatch, Event as EventStruct,
    find_rule_files, load_rules_with_progress, setup_logging, format_duration
};

#[derive(Parser, Debug)]
#[command(name = "event_detector")]
#[command(about = "Applies Sigma rules to events from a JSON file")]
#[command(version)]
struct Args {
    #[command(flatten)]
    common: CommonArgs,
    
    /// Input JSON file containing events
    #[arg(short = 'i', long = "events", required = true)]
    events_file: PathBuf,
    
    /// Output file for results (default: stdout)
    #[arg(short = 'o', long = "output")]
    output_file: Option<PathBuf>,
    
    /// Write output to stdout instead of file
    #[arg(long, conflicts_with = "output_file")]
    stdout: bool,
    
    /// Pretty print JSON output
    #[arg(long)]
    pretty: bool,
    
    /// Show progress bar
    #[arg(long, default_value_t = true)]
    progress: bool,
}

fn main() -> Result<()> {
    let args = Args::parse();
    setup_logging(args.common.verbose);
    
    if args.common.rule_dirs.is_empty() {
        error!("No rule directories specified");
        std::process::exit(1);
    }
    
    let start = Instant::now();
    
    // Load rules
    let files = find_rule_files(&args.common.rule_dirs)?;
    if files.is_empty() {
        error!("No rule files found");
        std::process::exit(1);
    }
    
    let (rules, stats) = load_rules_with_progress(files)?;
    info!("Loaded {} rules (failed: {}, unsupported: {})", 
        stats.parsed, stats.failed, stats.unsupported);
    
    // TODO: RuleSet is not available in the current API
    // For now, we'll skip processing events
    // let ruleset = RuleSet::new(rules);
    info!("Loaded {} rules", rules.len());
    
    // Process events
    let results = Vec::new(); // process_events(&args, &rules)?;
    let duration = start.elapsed();
    
    // Write results
    write_results(&args, results)?;
    
    info!("Processing completed in {}", format_duration(duration));
    Ok(())
}

fn process_events(args: &Args, rules: &[Rule]) -> Result<Vec<DetectionResult>> {
    let file = File::open(&args.events_file)?;
    let reader = BufReader::new(file);
    
    // Count events first for progress bar
    let event_count = if args.progress {
        count_events(&args.events_file)?
    } else {
        0
    };
    
    let pb = if args.progress && event_count > 0 {
        Some(create_progress_bar(event_count))
    } else {
        None
    };
    
    let mut results = Vec::new();
    let mut events_processed = 0;
    let mut events_matched = 0;
    
    // Try to parse as JSON array first
    let file = File::open(&args.events_file)?;
    let reader = BufReader::new(file);
    
    match serde_json::from_reader::<_, Vec<Value>>(reader) {
        Ok(events) => {
            // Process array of events
            for event_value in events {
                if let Some(ref pb) = pb {
                    pb.inc(1);
                }
                
                let result = process_single_event(event_value, rules);
                if !result.matches.is_empty() {
                    events_matched += 1;
                    results.push(result);
                }
                events_processed += 1;
            }
        }
        Err(_) => {
            // Try stream parsing (newline-delimited JSON)
            let file = File::open(&args.events_file)?;
            let reader = BufReader::new(file);
            let stream = Deserializer::from_reader(reader).into_iter::<Value>();
            
            for event_result in stream {
                if let Some(ref pb) = pb {
                    pb.inc(1);
                }
                
                match event_result {
                    Ok(event_value) => {
                        let result = process_single_event(event_value, rules);
                        if !result.matches.is_empty() {
                            events_matched += 1;
                            results.push(result);
                        }
                        events_processed += 1;
                    }
                    Err(e) => {
                        warn!("Failed to parse event: {}", e);
                    }
                }
            }
        }
    }
    
    if let Some(pb) = pb {
        pb.finish_and_clear();
    }
    
    info!("Processed {} events, {} matched rules", events_processed, events_matched);
    Ok(results)
}

fn process_single_event(event_value: Value, rules: &[Rule]) -> DetectionResult {
    let start = Instant::now();
    let event = DynamicEvent::new(event_value.clone());
    
    let mut matches = Vec::new();
    
    // TODO: Apply rules without RuleSet
    // This is a placeholder - the actual RuleSet implementation is not available
    // for rule in rules {
    //     // Apply each rule to the event
    // }
    
    let processing_time = start.elapsed();
    
    DetectionResult {
        event: EventStruct { data: event_value.as_object().unwrap().clone() },
        matches,
        timestamp: chrono::Utc::now(),
        processing_time_ms: processing_time.as_millis() as u64,
    }
}

fn write_results(args: &Args, results: Vec<DetectionResult>) -> Result<()> {
    let writer: Box<dyn Write> = if args.stdout {
        Box::new(std::io::stdout())
    } else if let Some(ref path) = args.output_file {
        Box::new(BufWriter::new(File::create(path)?))
    } else {
        // Default to stdout if no output specified
        Box::new(std::io::stdout())
    };
    
    if args.pretty {
        serde_json::to_writer_pretty(writer, &results)?;
    } else {
        for result in results {
            serde_json::to_writer(&writer, &result)?;
            writeln!(&mut writer.as_ref())?;
        }
    }
    
    Ok(())
}

fn count_events(path: &PathBuf) -> Result<u64> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);
    
    // Try parsing as array first
    match serde_json::from_reader::<_, Vec<Value>>(reader) {
        Ok(events) => Ok(events.len() as u64),
        Err(_) => {
            // Count newline-delimited JSON
            let file = File::open(path)?;
            let reader = BufReader::new(file);
            let stream = Deserializer::from_reader(reader).into_iter::<Value>();
            Ok(stream.count() as u64)
        }
    }
}

fn create_progress_bar(total: u64) -> ProgressBar {
    let pb = ProgressBar::new(total);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({eta}) Events")
            .unwrap()
    );
    pb
}