//! Rule validator - validates and reports on Sigma rule parsing
//! 
//! This example loads Sigma rules from specified directories and validates
//! their parsing, reporting statistics on success, failure, and unsupported features.
//! 
//! Usage:
//!     cargo run --example rule_validator -- --rule-dirs /path/to/rules
//!     cargo run --example rule_validator -- --rule-dirs /rules1;/rules2;/rules3 -vv
//!     cargo run --example rule_validator -- --rule-dirs ./sigma-rules --json

use clap::Parser;
use std::path::PathBuf;
use std::time::Instant;
use sigma_rs::error::Result;
use tracing::{info, error};
use rayon::prelude::*;

mod common;
use common::{CommonArgs, RuleStats, find_rule_files, setup_logging, format_duration};

#[derive(Parser, Debug)]
#[command(name = "rule_validator")]
#[command(about = "Validates Sigma rule parsing and reports statistics")]
#[command(version)]
struct Args {
    #[command(flatten)]
    common: CommonArgs,
    
    /// Use parallel processing for rule validation
    #[arg(long, default_value_t = true)]
    parallel: bool,
    
    /// Number of threads for parallel processing (0 = auto)
    #[arg(long, default_value_t = 0)]
    threads: usize,
}

fn main() -> Result<()> {
    let args = Args::parse();
    setup_logging(args.common.verbose);
    
    if args.common.rule_dirs.is_empty() {
        error!("No rule directories specified");
        std::process::exit(1);
    }
    
    // Configure rayon thread pool if specified
    if args.threads > 0 {
        rayon::ThreadPoolBuilder::new()
            .num_threads(args.threads)
            .build_global()
            .unwrap();
    }
    
    let start = Instant::now();
    
    // Find all rule files
    let files = find_rule_files(&args.common.rule_dirs)?;
    if files.is_empty() {
        error!("No rule files found in specified directories");
        std::process::exit(1);
    }
    
    info!("Validating {} rule files", files.len());
    
    // Validate rules
    let stats = if args.parallel {
        validate_rules_parallel(files)?
    } else {
        validate_rules_sequential(files)?
    };
    
    let duration = start.elapsed();
    
    // Output results
    if args.common.json {
        output_json(&stats, duration)?;
    } else {
        output_human(&stats, duration);
    }
    
    // Exit with error code if any failures
    if stats.failed > 0 {
        std::process::exit(1);
    }
    
    Ok(())
}

fn validate_rules_sequential(files: Vec<PathBuf>) -> Result<RuleStats> {
    use indicatif::{ProgressBar, ProgressStyle};
    use sigma_rs::rule::{Rule, rule_from_yaml};
    
    let mut stats = RuleStats::default();
    let pb = ProgressBar::new(files.len() as u64);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({eta}) {msg}")
            .unwrap()
    );
    
    for file in files {
        pb.inc(1);
        pb.set_message(format!("{}", file.file_name().unwrap_or_default().to_string_lossy()));
        
        match std::fs::read_to_string(&file) {
            Ok(content) => {
                match rule_from_yaml(content.as_bytes()) {
                    Ok(_rule) => {
                        stats.add_success();
                        info!("Validated: {}", file.display());
                    }
                    Err(e) => {
                        match &e {
                            sigma_rs::error::SigmaError::UnsupportedToken(_) => {
                                stats.add_unsupported();
                                info!("Unsupported: {} - {}", file.display(), e);
                            }
                            _ => {
                                stats.add_failure();
                                error!("Failed: {} - {}", file.display(), e);
                            }
                        }
                    }
                }
            }
            Err(e) => {
                stats.add_failure();
                error!("Failed to read: {} - {}", file.display(), e);
            }
        }
    }
    
    pb.finish_and_clear();
    Ok(stats)
}

fn validate_rules_parallel(files: Vec<PathBuf>) -> Result<RuleStats> {
    use std::sync::{Arc, Mutex};
    use indicatif::{ProgressBar, ProgressStyle};
    use sigma_rs::rule::{Rule, rule_from_yaml};
    
    let stats = Arc::new(Mutex::new(RuleStats::default()));
    let pb = Arc::new(ProgressBar::new(files.len() as u64));
    pb.set_style(
        ProgressStyle::default_bar()
            .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({eta})")
            .unwrap()
    );
    
    files.par_iter().for_each(|file| {
        pb.inc(1);
        
        let result = match std::fs::read_to_string(file) {
            Ok(content) => rule_from_yaml(content.as_bytes()),
            Err(e) => Err(sigma_rs::error::SigmaError::Io(e)),
        };
        
        let mut stats = stats.lock().unwrap();
        match result {
            Ok(_) => {
                stats.add_success();
                info!("Validated: {}", file.display());
            }
            Err(e) => {
                match &e {
                    sigma_rs::error::SigmaError::UnsupportedToken(_) => {
                        stats.add_unsupported();
                        info!("Unsupported: {} - {}", file.display(), e);
                    }
                    _ => {
                        stats.add_failure();
                        error!("Failed: {} - {}", file.display(), e);
                    }
                }
            }
        }
    });
    
    pb.finish_and_clear();
    let stats = Arc::try_unwrap(stats).unwrap().into_inner().unwrap();
    Ok(stats)
}

fn output_json(stats: &RuleStats, duration: std::time::Duration) -> Result<()> {
    use serde_json::json;
    
    let output = json!({
        "stats": stats,
        "duration_ms": duration.as_millis(),
        "success": stats.failed == 0,
    });
    
    tracing::error!("{}", serde_json::to_string_pretty(&output)?);
    Ok(())
}

fn output_human(stats: &RuleStats, duration: std::time::Duration) {
    tracing::error!("\nValidation Results:");
    tracing::error!("==================");
    tracing::error!("Total rules:      {}", stats.total);
    tracing::error!("Successfully parsed: {} ({:.1}%)", 
        stats.parsed, 
        (stats.parsed as f64 / stats.total as f64) * 100.0
    );
    tracing::error!("Failed to parse:    {} ({:.1}%)", 
        stats.failed,
        (stats.failed as f64 / stats.total as f64) * 100.0
    );
    tracing::error!("Unsupported:        {} ({:.1}%)", 
        stats.unsupported,
        (stats.unsupported as f64 / stats.total as f64) * 100.0
    );
    tracing::error!("\nProcessing time: {}", format_duration(duration));
    
    if stats.failed > 0 {
        tracing::error!("\n⚠️  Some rules failed to parse. Run with -v for details.");
    }
}