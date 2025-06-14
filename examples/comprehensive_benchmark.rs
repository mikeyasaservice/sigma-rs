//! Comprehensive benchmark that matches the Go study methodology
//! 
//! This loads the full Windows Sigma ruleset (2,301 rules) and benchmarks
//! event processing against realistic Windows event data, providing an
//! accurate comparison with the academic Go implementation.

use sigma_rs::{DynamicEvent, rule::{RuleHandle, rule_from_yaml}, tree::builder::build_tree};
use serde_json::Value;
use std::path::PathBuf;
use std::time::Instant;
use std::fs;
use std::io::{BufRead, BufReader};
use clap::{Arg, Command};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let matches = Command::new("Comprehensive Sigma Benchmark")
        .version("1.0")
        .about("Benchmarks Sigma rule engine against full Windows ruleset")
        .arg(Arg::new("rules-dir")
            .short('r')
            .long("rules")
            .value_name("DIRECTORY")
            .help("Directory containing Sigma rules")
            .default_value("rules/windows"))
        .arg(Arg::new("events-file")
            .short('e')
            .long("events")
            .value_name("FILE")
            .help("JSONL file containing test events")
            .default_value("sample_events.jsonl"))
        .arg(Arg::new("event-count")
            .short('c')
            .long("count")
            .value_name("NUMBER")
            .help("Number of events to process")
            .default_value("10000"))
        .get_matches();

    let rules_dir = matches.get_one::<String>("rules-dir").unwrap();
    let events_file = matches.get_one::<String>("events-file").unwrap();
    let event_count: usize = matches.get_one::<String>("event-count").unwrap().parse()?;

    println!("🔍 Comprehensive Sigma Rule Engine Benchmark");
    println!("===========================================");
    println!("Rules directory: {}", rules_dir);
    println!("Events file: {}", events_file);
    println!("Target event count: {}", event_count);
    println!();

    // Step 1: Load all Sigma rules
    println!("📋 Loading Sigma rules from {}...", rules_dir);
    let rule_files = collect_rule_files(rules_dir)?;
    println!("Found {} rule files", rule_files.len());

    let mut valid_rules = Vec::new();
    let mut failed_rules = 0;

    for rule_file in rule_files {
        match load_rule(&rule_file).await {
            Ok(rule_handle) => valid_rules.push(rule_handle),
            Err(e) => {
                failed_rules += 1;
                if failed_rules <= 5 {
                    println!("⚠️  Failed to load {}: {}", rule_file, e);
                }
            }
        }
    }

    println!("✅ Loaded {} valid rules ({} failed)", valid_rules.len(), failed_rules);
    if failed_rules > 5 {
        println!("   ... and {} more failed rules", failed_rules - 5);
    }

    // Step 2: Build trees for all rules
    println!("\n🌳 Building rule trees...");
    let start_build = Instant::now();
    
    let mut trees = Vec::new();
    let mut build_failures = 0;
    
    for rule_handle in valid_rules {
        match build_tree(rule_handle).await {
            Ok(tree) => trees.push(tree),
            Err(_) => build_failures += 1,
        }
    }
    
    let build_duration = start_build.elapsed();
    println!("✅ Built {} trees in {:?} ({} failures)", trees.len(), build_duration, build_failures);

    // Step 3: Load test events
    println!("\n📊 Loading test events from {}...", events_file);
    let events = load_events(events_file, event_count)?;
    println!("✅ Loaded {} events", events.len());

    // Step 4: Benchmark similar to Go study methodology
    println!("\n🚀 Running benchmark (Go study methodology)...");
    
    // First, establish JSON decode baseline (like Go study)
    let decode_start = Instant::now();
    for event_json in &events {
        let _: Value = serde_json::from_str(event_json)?;
    }
    let decode_duration = decode_start.elapsed();
    let decode_per_event = decode_duration / events.len() as u32;
    
    println!("\n📈 JSON Decode Baseline:");
    println!("  Total time: {:?}", decode_duration);
    println!("  Per event: {:?} ({:.0} ns)", decode_per_event, decode_per_event.as_nanos());
    println!("  Events/sec: {:.0}", 1.0 / decode_per_event.as_secs_f64());

    // Convert to DynamicEvents for processing
    let mut dynamic_events = Vec::new();
    for event_json in &events {
        let json_val: Value = serde_json::from_str(event_json)?;
        dynamic_events.push(DynamicEvent::new(json_val));
    }

    // Warm up
    println!("\n🔥 Warming up...");
    for _ in 0..100 {
        for tree in trees.iter().take(10) {
            for event in dynamic_events.iter().take(10) {
                let _ = tree.root.matches(event).await;
            }
        }
    }

    // Main benchmark: Process each event against ALL rules (like Go study)
    println!("\n⚡ Main benchmark: Full ruleset evaluation per event");
    println!("   Processing {} events against {} rules = {} total operations", 
             dynamic_events.len(), trees.len(), dynamic_events.len() * trees.len());

    let benchmark_start = Instant::now();
    let mut total_matches = 0;

    for event in &dynamic_events {
        for tree in &trees {
            let result = tree.root.matches(event).await;
            if result.matched {
                total_matches += 1;
            }
        }
    }

    let benchmark_duration = benchmark_start.elapsed();
    let total_operations = dynamic_events.len() * trees.len();
    let per_operation = benchmark_duration / total_operations as u32;
    let per_event = benchmark_duration / dynamic_events.len() as u32;

    println!("\n🏆 BENCHMARK RESULTS:");
    println!("====================");
    println!("Total operations: {}", total_operations);
    println!("Total matches found: {}", total_matches);
    println!("Total time: {:?}", benchmark_duration);
    println!();
    println!("📊 Per-operation performance:");
    println!("  Average: {:?} ({:.0} ns)", per_operation, per_operation.as_nanos());
    println!("  Operations/sec: {:.0}", 1.0 / per_operation.as_secs_f64());
    println!();
    println!("📊 Per-event performance (full ruleset):");
    println!("  Average: {:?} ({:.2} μs)", per_event, per_event.as_micros() as f64);
    println!("  Events/sec: {:.0}", 1.0 / per_event.as_secs_f64());

    // Compare with Go study baseline
    println!("\n🔄 Comparison with Go Study:");
    println!("============================");
    
    // Go study reported 1,363-1,494 ns per operation with built-in benchmarks
    let go_baseline_ns = 1400.0; // Average from study
    let rust_ns = per_operation.as_nanos() as f64;
    
    println!("Go study (per rule): {:.0} ns", go_baseline_ns);
    println!("Rust impl (per rule): {:.0} ns", rust_ns);
    
    if rust_ns < go_baseline_ns {
        let speedup = go_baseline_ns / rust_ns;
        println!("🚀 Rust is {:.2}x FASTER than Go!", speedup);
    } else {
        let slowdown = rust_ns / go_baseline_ns;
        println!("📉 Rust is {:.2}x slower than Go", slowdown);
    }

    // Go study also measured wall clock time for full ruleset (469 rules)
    // They reported ~4.5x penalty vs JSON decode alone
    let expected_go_per_event_us = (go_baseline_ns * 469.0) / 1000.0; // 469 rules in Go study
    let rust_per_event_us = per_event.as_micros() as f64;
    
    println!("\nFull ruleset comparison:");
    println!("Go study (469 rules): {:.0} μs per event", expected_go_per_event_us);
    println!("Rust ({} rules): {:.0} μs per event", trees.len(), rust_per_event_us);
    
    // Normalize for rule count difference
    let normalized_rust_us = rust_per_event_us * (469.0 / trees.len() as f64);
    println!("Rust normalized (469 rules): {:.0} μs per event", normalized_rust_us);
    
    if normalized_rust_us < expected_go_per_event_us {
        let speedup = expected_go_per_event_us / normalized_rust_us;
        println!("🎯 Normalized: Rust is {:.2}x FASTER than Go!", speedup);
    } else {
        let slowdown = normalized_rust_us / expected_go_per_event_us;
        println!("🎯 Normalized: Rust is {:.2}x slower than Go", slowdown);
    }

    // Performance breakdown
    println!("\n📈 Performance Breakdown:");
    println!("=========================");
    println!("JSON decode overhead: {:.0} ns per event", decode_per_event.as_nanos());
    println!("Rule processing: {:.0} μs per event", per_event.as_micros() as f64);
    let overhead_ratio = decode_per_event.as_nanos() as f64 / per_event.as_nanos() as f64 * 100.0;
    println!("JSON decode as % of total: {:.1}%", overhead_ratio);

    println!("\n✨ Benchmark completed successfully!");
    
    Ok(())
}

fn collect_rule_files(rules_dir: &str) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let mut rule_files = Vec::new();
    
    fn collect_recursive(dir: &str, files: &mut Vec<String>) -> Result<(), Box<dyn std::error::Error>> {
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            
            if path.is_dir() {
                collect_recursive(&path.to_string_lossy(), files)?;
            } else if let Some(ext) = path.extension() {
                if ext == "yml" || ext == "yaml" {
                    files.push(path.to_string_lossy().to_string());
                }
            }
        }
        Ok(())
    }
    
    collect_recursive(rules_dir, &mut rule_files)?;
    Ok(rule_files)
}

async fn load_rule(file_path: &str) -> Result<RuleHandle, Box<dyn std::error::Error>> {
    let content = fs::read_to_string(file_path)?;
    let rule = rule_from_yaml(content.as_bytes())?;
    Ok(RuleHandle::new(rule, PathBuf::from(file_path)))
}

fn load_events(file_path: &str, max_count: usize) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let file = fs::File::open(file_path)?;
    let reader = BufReader::new(file);
    
    let mut events = Vec::new();
    for (i, line) in reader.lines().enumerate() {
        if i >= max_count {
            break;
        }
        events.push(line?);
    }
    
    Ok(events)
}