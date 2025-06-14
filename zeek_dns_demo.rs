use sigma_rs::{DynamicEvent, RuleSet};
use serde_json::json;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter("sigma_rs=debug")
        .init();

    // Create a Sigma rule for DNS detection
    let dns_rule = r#"
title: Suspicious DNS Queries
id: test-dns-001
status: experimental
description: Detects suspicious DNS queries
logsource:
    product: zeek
    service: dns
detection:
    selection:
        zeek.dns.query|contains:
            - 'malicious-domain'
            - 'crypto-miner'
            - 'tor2web'
    condition: selection
level: high
"#;

    // Parse the rule
    let rule = sigma_rs::rule::rule_from_yaml(dns_rule.as_bytes())?;
    
    // Create a ruleset and add the rule
    let mut ruleset = RuleSet::new();
    ruleset.add_rule(rule).await?;
    
    println!("Loaded {} rule(s)", ruleset.len());

    // Create sample Zeek DNS events
    let events = vec![
        json!({
            "@timestamp": "2025-01-22T10:15:23.000Z",
            "event.module": "zeek",
            "event.dataset": "zeek.dns",
            "zeek.dns.query": "malicious-domain.com",
            "zeek.dns.id.orig_h": "192.168.1.100",
            "zeek.dns.id.resp_h": "8.8.8.8",
        }),
        json!({
            "@timestamp": "2025-01-22T10:15:24.000Z",
            "event.module": "zeek",
            "event.dataset": "zeek.dns",
            "zeek.dns.query": "google.com",
            "zeek.dns.id.orig_h": "192.168.1.101",
            "zeek.dns.id.resp_h": "8.8.8.8",
        }),
        json!({
            "@timestamp": "2025-01-22T10:15:25.000Z",
            "event.module": "zeek", 
            "event.dataset": "zeek.dns",
            "zeek.dns.query": "suspicious-crypto-miner.tk",
            "zeek.dns.id.orig_h": "192.168.1.105",
            "zeek.dns.id.resp_h": "8.8.8.8",
        }),
        json!({
            "@timestamp": "2025-01-22T10:15:26.000Z",
            "event.module": "zeek",
            "event.dataset": "zeek.dns",
            "zeek.dns.query": "tor2web.org",
            "zeek.dns.id.orig_h": "192.168.1.100",
            "zeek.dns.id.resp_h": "8.8.8.8", 
        }),
    ];

    // Process each event
    println!("\nProcessing {} events...\n", events.len());
    
    for (idx, event_json) in events.iter().enumerate() {
        let event = DynamicEvent::new(event_json.clone());
        let result = ruleset.evaluate(&event).await?;
        
        let query = event_json["zeek.dns.query"].as_str().unwrap_or("unknown");
        let src_ip = event_json["zeek.dns.id.orig_h"].as_str().unwrap_or("unknown");
        
        println!("Event {}: DNS query '{}' from {}", idx + 1, query, src_ip);
        
        for match_result in &result.matches {
            if match_result.matched {
                println!("  ✓ MATCHED: {} (level: {})", 
                    match_result.rule_title, 
                    match_result.rule_level
                );
            }
        }
        
        if result.matches.is_empty() || !result.matches.iter().any(|m| m.matched) {
            println!("  ✗ No matches");
        }
        
        println!();
    }

    Ok(())
}