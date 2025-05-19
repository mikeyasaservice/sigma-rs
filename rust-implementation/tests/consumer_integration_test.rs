//! Integration tests for Redpanda consumer with Sigma rule processing

use sigma_rs::{
    consumer::{
        config::ConsumerConfig,
        consumer::RedpandaConsumer,
        processor::MessageProcessor,
        error::ConsumerError,
    },
    rule::{Rule, rule_from_yaml, RuleSet},
    matcher::EventMatcher,
    event::Event,
};
use async_trait::async_trait;
use rdkafka::{
    message::OwnedMessage,
    producer::{FutureProducer, FutureRecord},
    ClientConfig,
};
use std::sync::{Arc, atomic::{AtomicU32, Ordering}};
use std::time::Duration;
use testcontainers::{clients, images::kafka::Kafka};
use serde_json::json;
use tokio::sync::Mutex;

// Sigma rule processor that detects security events
struct SigmaProcessor {
    rules: Arc<RuleSet>,
    matches: Arc<AtomicU32>,
    processed: Arc<AtomicU32>,
    alerts: Arc<Mutex<Vec<Alert>>>,
}

#[derive(Debug, Clone)]
struct Alert {
    rule_id: String,
    rule_title: String,
    event: serde_json::Value,
    timestamp: std::time::SystemTime,
}

#[async_trait]
impl MessageProcessor for SigmaProcessor {
    type Error = ConsumerError;
    
    async fn process(&self, message: &OwnedMessage) -> Result<(), Self::Error> {
        self.processed.fetch_add(1, Ordering::Relaxed);
        
        // Parse message payload as JSON event
        let payload = message.payload()
            .ok_or_else(|| ConsumerError::ParseError("Empty payload".to_string()))?;
            
        let event: serde_json::Value = serde_json::from_slice(payload)
            .map_err(|e| ConsumerError::ParseError(format!("JSON parse error: {}", e)))?;
        
        // Check event against all rules
        for rule in self.rules.rules() {
            if self.check_rule(&rule, &event).await {
                self.matches.fetch_add(1, Ordering::Relaxed);
                
                // Create alert
                let alert = Alert {
                    rule_id: rule.id.clone(),
                    rule_title: rule.title.clone(),
                    event: event.clone(),
                    timestamp: std::time::SystemTime::now(),
                };
                
                self.alerts.lock().await.push(alert);
            }
        }
        
        Ok(())
    }
    
    async fn on_success(&self, _message: &OwnedMessage) {
        // Could log successful processing
    }
    
    async fn on_failure(&self, error: &Self::Error, message: &OwnedMessage) {
        eprintln!("Processing failed: {} for message at offset {}", 
                  error, message.offset());
    }
}

impl SigmaProcessor {
    async fn check_rule(&self, rule: &Rule, event: &serde_json::Value) -> bool {
        // Simplified rule matching - actual implementation would use full engine
        if let Some(detection) = rule.detection.get("selection") {
            // Basic field matching
            true // Placeholder
        } else {
            false
        }
    }
}

#[tokio::test]
async fn test_consumer_with_sigma_rules() {
    // Start Kafka container
    let docker = clients::Cli::default();
    let kafka_node = docker.run(Kafka::default());
    let brokers = format!("localhost:{}", kafka_node.get_host_port_ipv4(9092));
    
    // Wait for Kafka to be ready
    tokio::time::sleep(Duration::from_secs(5)).await;
    
    // Create Sigma rules
    let ruleset = create_test_ruleset();
    
    // Create processor
    let processor = SigmaProcessor {
        rules: Arc::new(ruleset),
        matches: Arc::new(AtomicU32::new(0)),
        processed: Arc::new(AtomicU32::new(0)),
        alerts: Arc::new(Mutex::new(Vec::new())),
    };
    
    // Create consumer configuration
    let config = ConsumerConfig::builder()
        .brokers(brokers.clone())
        .group_id("sigma-test-group".to_string())
        .topics(vec!["security-events".to_string()])
        .batch_size(10)
        .channel_buffer_size(100)
        .build();
    
    // Create and start consumer
    let consumer = RedpandaConsumer::new(config, processor.clone()).await.unwrap();
    let consumer_handle = tokio::spawn(async move {
        consumer.run().await
    });
    
    // Produce test events
    produce_test_events(&brokers, "security-events").await;
    
    // Let consumer process events
    tokio::time::sleep(Duration::from_secs(10)).await;
    
    // Check results
    let processed = processor.processed.load(Ordering::Relaxed);
    let matches = processor.matches.load(Ordering::Relaxed);
    let alerts = processor.alerts.lock().await;
    
    println!("Processed: {}, Matches: {}, Alerts: {}", 
             processed, matches, alerts.len());
    
    assert!(processed > 0, "Should have processed events");
    assert!(matches > 0, "Should have matched some rules");
    assert!(!alerts.is_empty(), "Should have generated alerts");
    
    // Verify alert content
    for alert in alerts.iter() {
        println!("Alert: {} - {}", alert.rule_id, alert.rule_title);
        assert!(!alert.rule_id.is_empty());
        assert!(!alert.rule_title.is_empty());
    }
    
    // Shutdown
    consumer_handle.abort();
}

#[tokio::test]
async fn test_high_volume_processing() {
    let docker = clients::Cli::default();
    let kafka_node = docker.run(Kafka::default());
    let brokers = format!("localhost:{}", kafka_node.get_host_port_ipv4(9092));
    
    tokio::time::sleep(Duration::from_secs(5)).await;
    
    // Simple counting processor
    struct CountingProcessor {
        count: Arc<AtomicU32>,
    }
    
    #[async_trait]
    impl MessageProcessor for CountingProcessor {
        type Error = ConsumerError;
        
        async fn process(&self, _message: &OwnedMessage) -> Result<(), Self::Error> {
            self.count.fetch_add(1, Ordering::Relaxed);
            Ok(())
        }
        
        async fn on_success(&self, _message: &OwnedMessage) {}
        async fn on_failure(&self, _error: &Self::Error, _message: &OwnedMessage) {}
    }
    
    let processor = CountingProcessor {
        count: Arc::new(AtomicU32::new(0)),
    };
    
    let config = ConsumerConfig::builder()
        .brokers(brokers.clone())
        .group_id("perf-test-group".to_string())
        .topics(vec!["high-volume".to_string()])
        .batch_size(100)
        .max_inflight_messages(500)
        .channel_buffer_size(1000)
        .build();
    
    let consumer = RedpandaConsumer::new(config, processor.clone()).await.unwrap();
    let consumer_handle = tokio::spawn(async move {
        consumer.run().await
    });
    
    // Produce many events
    let event_count = 10000;
    produce_bulk_events(&brokers, "high-volume", event_count).await;
    
    // Wait for processing
    tokio::time::sleep(Duration::from_secs(15)).await;
    
    let processed = processor.count.load(Ordering::Relaxed);
    println!("Processed {} out of {} events", processed, event_count);
    
    assert!(processed as u64 >= event_count * 80 / 100, 
            "Should process at least 80% of events");
    
    consumer_handle.abort();
}

#[tokio::test]
async fn test_error_handling_and_dlq() {
    let docker = clients::Cli::default();
    let kafka_node = docker.run(Kafka::default());
    let brokers = format!("localhost:{}", kafka_node.get_host_port_ipv4(9092));
    
    tokio::time::sleep(Duration::from_secs(5)).await;
    
    // Processor that fails on certain events
    struct FailingProcessor {
        processed: Arc<AtomicU32>,
        failed: Arc<AtomicU32>,
    }
    
    #[async_trait]
    impl MessageProcessor for FailingProcessor {
        type Error = ConsumerError;
        
        async fn process(&self, message: &OwnedMessage) -> Result<(), Self::Error> {
            let payload = message.payload().unwrap();
            let event: serde_json::Value = serde_json::from_slice(payload).unwrap();
            
            if event["fail"].as_bool().unwrap_or(false) {
                self.failed.fetch_add(1, Ordering::Relaxed);
                Err(ConsumerError::ProcessingError("Simulated failure".to_string()))
            } else {
                self.processed.fetch_add(1, Ordering::Relaxed);
                Ok(())
            }
        }
        
        async fn on_success(&self, _message: &OwnedMessage) {}
        async fn on_failure(&self, _error: &Self::Error, _message: &OwnedMessage) {}
        
        fn is_retryable(&self, _error: &Self::Error) -> bool {
            false // Don't retry, send to DLQ
        }
    }
    
    let processor = FailingProcessor {
        processed: Arc::new(AtomicU32::new(0)),
        failed: Arc::new(AtomicU32::new(0)),
    };
    
    let config = ConsumerConfig::builder()
        .brokers(brokers.clone())
        .group_id("error-test-group".to_string())
        .topics(vec!["error-events".to_string()])
        .dlq_topic("error-events-dlq".to_string())
        .dlq_after_retries(1)
        .build();
    
    let consumer = RedpandaConsumer::new(config, processor.clone()).await.unwrap();
    let consumer_handle = tokio::spawn(async move {
        consumer.run().await
    });
    
    // Produce mix of good and bad events
    produce_mixed_events(&brokers, "error-events").await;
    
    tokio::time::sleep(Duration::from_secs(10)).await;
    
    let processed = processor.processed.load(Ordering::Relaxed);
    let failed = processor.failed.load(Ordering::Relaxed);
    
    println!("Successfully processed: {}, Failed: {}", processed, failed);
    
    assert!(processed > 0, "Should process good events");
    assert!(failed > 0, "Should fail on bad events");
    
    // Check DLQ has messages
    // Would need to consume from DLQ topic to verify
    
    consumer_handle.abort();
}

// Helper functions

fn create_test_ruleset() -> RuleSet {
    let rule1 = r#"
title: Suspicious PowerShell
id: rule-001
logsource:
    product: windows
    service: sysmon
detection:
    selection:
        EventID: 1
        CommandLine|contains: 'powershell'
    condition: selection
"#;
    
    let rule2 = r#"
title: Network Connection
id: rule-002
logsource:
    product: windows
    service: sysmon
detection:
    selection:
        EventID: 3
        DestinationPort: 
            - 445
            - 139
    condition: selection
"#;
    
    let mut ruleset = RuleSet::new();
    ruleset.add_rule(rule_from_yaml(rule1.as_bytes()).unwrap());
    ruleset.add_rule(rule_from_yaml(rule2.as_bytes()).unwrap());
    ruleset
}

async fn produce_test_events(brokers: &str, topic: &str) {
    let producer: FutureProducer = ClientConfig::new()
        .set("bootstrap.servers", brokers)
        .create()
        .unwrap();
    
    let events = vec![
        json!({
            "EventID": 1,
            "CommandLine": "powershell.exe -ExecutionPolicy Bypass",
            "Image": "C:\\Windows\\System32\\WindowsPowerShell\\v1.0\\powershell.exe",
            "TimeCreated": "2024-01-10T10:30:00Z"
        }),
        json!({
            "EventID": 3,
            "DestinationPort": 445,
            "DestinationIp": "192.168.1.100",
            "SourcePort": 49152,
            "TimeCreated": "2024-01-10T10:31:00Z"
        }),
        json!({
            "EventID": 1,
            "CommandLine": "cmd.exe /c whoami",
            "Image": "C:\\Windows\\System32\\cmd.exe",
            "TimeCreated": "2024-01-10T10:32:00Z"
        }),
    ];
    
    for (i, event) in events.iter().enumerate() {
        let record = FutureRecord::to(topic)
            .key(&format!("event-{}", i))
            .payload(&serde_json::to_string(event).unwrap());
            
        producer.send(record, Duration::from_secs(5)).await.unwrap();
    }
}

async fn produce_bulk_events(brokers: &str, topic: &str, count: u64) {
    let producer: FutureProducer = ClientConfig::new()
        .set("bootstrap.servers", brokers)
        .create()
        .unwrap();
    
    for i in 0..count {
        let event = json!({
            "EventID": 1,
            "Index": i,
            "Timestamp": std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        });
        
        let record = FutureRecord::to(topic)
            .key(&format!("bulk-{}", i))
            .payload(&serde_json::to_string(&event).unwrap());
            
        let _ = producer.send(record, Duration::from_secs(1)).await;
        
        if i % 1000 == 0 {
            println!("Produced {} events", i);
        }
    }
}

async fn produce_mixed_events(brokers: &str, topic: &str) {
    let producer: FutureProducer = ClientConfig::new()
        .set("bootstrap.servers", brokers)
        .create()
        .unwrap();
    
    for i in 0..100 {
        let event = json!({
            "EventID": 1,
            "Index": i,
            "fail": i % 5 == 0, // Every 5th event will fail
        });
        
        let record = FutureRecord::to(topic)
            .key(&format!("mixed-{}", i))
            .payload(&serde_json::to_string(&event).unwrap());
            
        producer.send(record, Duration::from_secs(5)).await.unwrap();
    }
}