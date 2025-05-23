//! Common utilities for Sigma rule engine examples

use std::collections::HashMap;
use std::path::PathBuf;
use std::fs;
use walkdir::WalkDir;
use serde::{Deserialize, Serialize};
use clap::Parser;
use indicatif::{ProgressBar, ProgressStyle};
use tracing::{info, warn, error};
use sigma_rs::rule::Rule;
use sigma_rs::error::Result;

/// Common CLI arguments shared across examples
#[derive(Parser, Debug)]
pub struct CommonArgs {
    /// Directories containing Sigma rules (semicolon-delimited)
    #[arg(long, value_delimiter = ';')]
    pub rule_dirs: Vec<PathBuf>,
    
    /// Verbosity level (can be specified multiple times)
    #[arg(short, long, action = clap::ArgAction::Count)]
    pub verbose: u8,
    
    /// Enable JSON output format
    #[arg(long)]
    pub json: bool,
}

/// Event structure for parsing log events
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event {
    #[serde(flatten)]
    pub data: HashMap<String, serde_json::Value>,
}

/// Detection result with metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetectionResult {
    pub event: Event,
    pub matches: Vec<RuleMatch>,
    #[serde(with = "chrono::serde::ts_seconds")]
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub processing_time_ms: u64,
}

/// Individual rule match information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuleMatch {
    pub rule_id: String,
    pub rule_name: String,
    pub rule_path: String,
    pub severity: String,
    pub tags: Vec<String>,
    pub score: f64,
}

/// Rule parsing statistics
#[derive(Debug, Default, Serialize)]
pub struct RuleStats {
    pub total: usize,
    pub parsed: usize,
    pub failed: usize,
    pub unsupported: usize,
}

impl RuleStats {
    pub fn add_success(&mut self) {
        self.total += 1;
        self.parsed += 1;
    }
    
    pub fn add_failure(&mut self) {
        self.total += 1;
        self.failed += 1;
    }
    
    pub fn add_unsupported(&mut self) {
        self.total += 1;
        self.unsupported += 1;
    }
}

/// Find all YAML rule files in the given directories
pub fn find_rule_files(dirs: &[PathBuf]) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    
    for dir in dirs {
        if !dir.exists() {
            warn!("Directory does not exist: {:?}", dir);
            continue;
        }
        
        for entry in WalkDir::new(dir)
            .follow_links(true)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let path = entry.path();
            if path.is_file() && (path.extension() == Some("yml".as_ref()) || 
                                path.extension() == Some("yaml".as_ref())) {
                files.push(path.to_path_buf());
            }
        }
    }
    
    info!("Found {} rule files", files.len());
    Ok(files)
}

/// Load rules from the given files with progress reporting
pub fn load_rules_with_progress(files: Vec<PathBuf>) -> Result<(Vec<Rule>, RuleStats)> {
    let mut rules = Vec::new();
    let mut stats = RuleStats::default();
    
    let pb = ProgressBar::new(files.len() as u64);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({eta})")
            .unwrap()
            .tick_chars("⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏"),
    );
    
    for file in files {
        pb.inc(1);
        pb.set_message(format!("Loading: {}", file.display()));
        
        match load_rule_from_file(&file) {
            Ok(rule) => {
                rules.push(rule);
                stats.add_success();
            }
            Err(e) => {
                // Check if it's an unsupported feature error
                if e.to_string().contains("Unsupported") {
                    stats.add_unsupported();
                    warn!("Unsupported rule: {} - {}", file.display(), e);
                } else {
                    stats.add_failure();
                    error!("Failed to parse rule: {} - {}", file.display(), e);
                }
            }
        }
    }
    
    pb.finish_with_message("Rules loaded");
    Ok((rules, stats))
}

fn load_rule_from_file(path: &PathBuf) -> Result<Rule> {
    let content = fs::read_to_string(path)?;
    let rule = match sigma_rs::rule::rule_from_yaml(content.as_bytes()) {
        Ok(r) => r,
        Err(e) => return Err(sigma_rs::SigmaError::YamlParse(e)),
    };
    Ok(rule)
}

/// Setup logging based on verbosity level
pub fn setup_logging(verbosity: u8) {
    let log_level = match verbosity {
        0 => tracing::Level::ERROR,
        1 => tracing::Level::WARN,
        2 => tracing::Level::INFO,
        3 => tracing::Level::DEBUG,
        _ => tracing::Level::TRACE,
    };
    
    tracing_subscriber::fmt()
        .with_max_level(log_level)
        .with_thread_ids(true)
        // Use default timer
        // .with_timer(tracing_subscriber::fmt::time::UtcTime::rfc_3339())
        .init();
}

/// Format duration in human-readable format
pub fn format_duration(duration: std::time::Duration) -> String {
    let secs = duration.as_secs();
    let millis = duration.subsec_millis();
    
    if secs > 0 {
        format!("{}.{:03}s", secs, millis)
    } else {
        format!("{}ms", duration.as_millis())
    }
}

/// Create a progress bar with standard style
pub fn create_progress_bar(total: u64, message: &str) -> ProgressBar {
    let pb = ProgressBar::new(total);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({eta}) {msg}")
            .unwrap()
            .tick_chars("⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏"),
    );
    pb.set_message(message.to_string());
    pb
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use std::fs;
    
    #[test]
    fn test_find_rule_files() {
        let temp_dir = TempDir::new().unwrap();
        let rule_dir = temp_dir.path().join("rules");
        fs::create_dir(&rule_dir).unwrap();
        
        // Create test files
        fs::write(rule_dir.join("test1.yml"), "title: Test1").unwrap();
        fs::write(rule_dir.join("test2.yaml"), "title: Test2").unwrap();
        fs::write(rule_dir.join("test3.txt"), "not a yaml").unwrap();
        
        let files = find_rule_files(&[rule_dir]).unwrap();
        assert_eq!(files.len(), 2);
    }
    
    #[test]
    fn test_rule_stats() {
        let mut stats = RuleStats::default();
        assert_eq!(stats.total, 0);
        
        stats.add_success();
        assert_eq!(stats.total, 1);
        assert_eq!(stats.parsed, 1);
        
        stats.add_failure();
        assert_eq!(stats.total, 2);
        assert_eq!(stats.failed, 1);
        
        stats.add_unsupported();
        assert_eq!(stats.total, 3);
        assert_eq!(stats.unsupported, 1);
    }
    
    #[test]
    fn test_format_duration() {
        use std::time::Duration;
        
        assert_eq!(format_duration(Duration::from_millis(500)), "500ms");
        assert_eq!(format_duration(Duration::from_millis(1500)), "1.500s");
        assert_eq!(format_duration(Duration::from_secs(30)), "30.000s");
    }
}