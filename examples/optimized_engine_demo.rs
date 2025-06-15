//! Demonstration of the optimized Sigma engine architecture
//!
//! This example shows how the tiered rule compiler and grouped pattern matching
//! work together to achieve high-performance event processing.

use sigma_rs::ast::tiered_compiler::{TieredCompiler, TieredRule};
use sigma_rs::engine::optimized_batch_processor::{
    OptimizedBatchProcessor, BatchProcessorConfig, OPTIMAL_BATCH_SIZE
};
use sigma_rs::pattern::grouped_matcher::{Pattern, PatternType};
use sigma_rs::rule::{Rule, Detection};
use serde_json::json;
use arrow::array::{RecordBatch, StringArray};
use arrow::datatypes::{Schema, Field, DataType};
use std::sync::Arc;
use std::collections::HashMap;
use std::time::Instant;
use tokio;

/// Create sample Sigma rules that represent common security detections
fn create_sample_rules() -> Vec<Rule> {
    vec![
        Rule {
            id: "detect_powershell_encoding".to_string(),
            title: "Encoded PowerShell Command".to_string(),
            description: Some("Detects encoded PowerShell commands".to_string()),
            level: Some("high".to_string()),
            detection: {
                let mut detection = Detection::new();
                detection.insert("condition".to_string(), json!("selection"));
                detection.insert("selection".to_string(), json!({
                    "CommandLine|contains": ["powershell", "encoded"]
                }));
                detection
            },
            tags: vec!["attack.execution".to_string()],
            logsource: Default::default(),
            author: None,
            falsepositives: vec![],
            fields: vec![],
            status: None,
            references: vec![],
            date: None,
            modified: None,
        },
        Rule {
            id: "detect_suspicious_process".to_string(),
            title: "Suspicious Process Execution".to_string(),
            description: Some("Detects suspicious process execution patterns".to_string()),
            level: Some("medium".to_string()),
            detection: {
                let mut detection = Detection::new();
                detection.insert("condition".to_string(), json!("selection"));
                detection.insert("selection".to_string(), json!({
                    "CommandLine|contains": "whoami",
                    "Image|endswith": ".exe"
                }));
                detection
            },
            tags: vec!["attack.discovery".to_string()],
            logsource: Default::default(),
            author: None,
            falsepositives: vec![],
            fields: vec![],
            status: None,
            references: vec![],
            date: None,
            modified: None,
        },
    ]
}

/// Generate test events
fn generate_test_events(count: usize) -> Vec<String> {
    let templates = vec![
        r#"{"EventID":1,"CommandLine":"powershell.exe -encoded SGVsbG8=","Image":"C:\\Windows\\System32\\WindowsPowerShell\\v1.0\\powershell.exe","User":"Admin"}"#,
        r#"{"EventID":1,"CommandLine":"cmd.exe /c whoami","Image":"C:\\Windows\\System32\\cmd.exe","User":"User1"}"#,
        r#"{"EventID":1,"CommandLine":"notepad.exe file.txt","Image":"C:\\Windows\\System32\\notepad.exe","User":"User2"}"#,
        r#"{"EventID":4688,"CommandLine":"net user admin /add","Image":"C:\\Windows\\System32\\net.exe","User":"SYSTEM"}"#,
    ];
    
    (0..count)
        .map(|i| templates[i % templates.len()].to_string())
        .collect()
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();
    
    println!("=== Optimized Sigma Engine Demo ===\n");
    
    // Step 1: Create and compile rules
    println!("1. Compiling rules with tiered compiler...");
    let mut compiler = TieredCompiler::new();
    
    // Manually add patterns for demo (in real implementation, compiler extracts these)
    let pattern_matcher = Arc::get_mut(&mut compiler.get_pattern_matcher()).unwrap();
    
    pattern_matcher.add_pattern("CommandLine", Pattern {
        pattern: "powershell".to_string(),
        pattern_type: PatternType::Contains,
        rule_id: "detect_powershell_encoding".to_string(),
        pattern_id: 0,
    })?;
    
    pattern_matcher.add_pattern("CommandLine", Pattern {
        pattern: "encoded".to_string(),
        pattern_type: PatternType::Contains,
        rule_id: "detect_powershell_encoding".to_string(),
        pattern_id: 1,
    })?;
    
    pattern_matcher.add_pattern("CommandLine", Pattern {
        pattern: "whoami".to_string(),
        pattern_type: PatternType::Contains,
        rule_id: "detect_suspicious_process".to_string(),
        pattern_id: 2,
    })?;
    
    pattern_matcher.add_pattern("Image", Pattern {
        pattern: ".exe".to_string(),
        pattern_type: PatternType::EndsWith,
        rule_id: "detect_suspicious_process".to_string(),
        pattern_id: 3,
    })?;
    
    // Compile rules
    for rule in create_sample_rules() {
        compiler.compile_rule(&rule)?;
    }
    compiler.build()?;
    
    let stats = compiler.stats();
    println!("   - Total rules: {}", stats.total_rules);
    println!("   - Simple rules: {}", stats.simple_rules);
    println!("   - Pattern rules: {}", stats.pattern_rules);
    println!("   - Complex rules: {}", stats.complex_rules);
    println!("   - Total patterns: {}", stats.total_patterns);
    
    // Step 2: Create optimized batch processor
    println!("\n2. Creating optimized batch processor...");
    let config = BatchProcessorConfig {
        batch_size: 10000, // Smaller for demo
        ..Default::default()
    };
    
    let processor = OptimizedBatchProcessor::new(config, Arc::new(compiler));
    
    // Step 3: Generate test events
    println!("\n3. Generating test events...");
    let event_count = 10000;
    let events = generate_test_events(event_count);
    println!("   - Generated {} events", event_count);
    
    // Step 4: Convert to Arrow format
    println!("\n4. Converting events to Arrow format...");
    let start = Instant::now();
    
    // Create schema
    let schema = Arc::new(Schema::new(vec![
        Field::new("EventID", DataType::Int64, true),
        Field::new("CommandLine", DataType::Utf8, true),
        Field::new("Image", DataType::Utf8, true),
        Field::new("User", DataType::Utf8, true),
    ]));
    
    // Parse events and create arrays
    let mut event_ids = Vec::new();
    let mut command_lines = Vec::new();
    let mut images = Vec::new();
    let mut users = Vec::new();
    
    for event_str in &events {
        let json: serde_json::Value = serde_json::from_str(event_str)?;
        event_ids.push(json["EventID"].as_i64());
        command_lines.push(json["CommandLine"].as_str());
        images.push(json["Image"].as_str());
        users.push(json["User"].as_str());
    }
    
    let batch = RecordBatch::try_new(
        schema,
        vec![
            Arc::new(arrow::array::Int64Array::from(event_ids)),
            Arc::new(StringArray::from(command_lines)),
            Arc::new(StringArray::from(images)),
            Arc::new(StringArray::from(users)),
        ],
    )?;
    
    let conversion_time = start.elapsed();
    println!("   - Conversion completed in {:?}", conversion_time);
    
    // Step 5: Process batch
    println!("\n5. Processing events...");
    let start = Instant::now();
    
    let results = processor.process_batch(batch).await?;
    
    let processing_time = start.elapsed();
    println!("   - Processing completed in {:?}", processing_time);
    
    // Step 6: Display results
    println!("\n6. Results:");
    println!("   - Events processed: {}", results.stats.events_processed);
    println!("   - Rules evaluated: {}", results.stats.rules_evaluated);
    println!("   - Matches found: {}", results.stats.matches_found);
    println!("   - Pattern matching time: {}ms", results.stats.pattern_matching_time_ms);
    println!("   - Rule evaluation time: {}ms", results.stats.rule_evaluation_time_ms);
    println!("   - Total processing time: {}ms", results.stats.processing_time_ms);
    
    // Calculate throughput
    let events_per_second = (event_count as f64 / processing_time.as_secs_f64()) as u64;
    println!("\n   - Throughput: {} events/second", events_per_second);
    
    // Show matched rules
    if !results.rule_matches.is_empty() {
        println!("\n7. Matched rules:");
        for (rule_id, indices) in &results.rule_matches {
            println!("   - Rule '{}': {} matches", rule_id, indices.len());
        }
    }
    
    println!("\n=== Demo Complete ===");
    println!("This demonstrates the core architecture for achieving 1M+ events/sec.");
    println!("For production use, implement:");
    println!("- Proper rule parsing and pattern extraction");
    println!("- Full AST to pattern conversion");
    println!("- Distributed processing for 3M+ events/sec");
    
    Ok(())
}