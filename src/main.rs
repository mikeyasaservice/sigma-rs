use clap::Parser;
use sigma_rs::{RuleSet, DynamicEvent};
use std::path::PathBuf;
use tracing_subscriber;
use serde_json::Value;
use std::io::{self, BufRead};

#[derive(Parser)]
#[command(name = "sigma-rs")]
#[command(about = "High-performance Sigma rule engine", long_about = None)]
struct Cli {
    /// Path to rules directory
    #[arg(short, long)]
    rules: PathBuf,
    
    /// Enable debug logging
    #[arg(short, long)]
    debug: bool,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    
    // Setup logging
    if cli.debug {
        tracing_subscriber::fmt::init();
    } else {
        tracing_subscriber::fmt()
            .with_max_level(tracing::Level::WARN)
            .init();
    }
    
    // Load rules
    let mut ruleset = RuleSet::new();
    ruleset.load_directory(&cli.rules.to_string_lossy()).await?;
    
    eprintln!("Loaded {} rules", ruleset.len());
    
    // Process events from stdin
    let stdin = io::stdin();
    for line in stdin.lock().lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        
        let event: Value = serde_json::from_str(&line)?;
        let dynamic_event = DynamicEvent::new(event.clone());
        
        let result = ruleset.evaluate(&dynamic_event).await?;
        
        for rule_match in &result.matches {
            if rule_match.matched {
                let output = serde_json::json!({
                    "event": event,
                    "rule_id": rule_match.rule_id,
                    "rule_title": rule_match.rule_title,
                });
                println!("{}", serde_json::to_string(&output)?);
            }
        }
    }
    
    Ok(())
}