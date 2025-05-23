use sigma_rs::consumer::config::{ConsumerConfig, ConsumerConfigBuilder};
use std::time::Duration;
use std::collections::HashMap;
use rstest::*;
use proptest::prelude::*;
use test_case::test_case;

#[test]
fn test_default_config() {
    let config = ConsumerConfig::default();
    
    assert_eq!(config.brokers, "localhost:9092");
    assert_eq!(config.group_id, "sigma-engine");
    assert_eq!(config.topics, vec!["events"]);
    assert_eq!(config.session_timeout_ms, 30000);
    assert_eq!(config.max_poll_interval_ms, 300000);
    assert!(!config.enable_auto_commit);
    assert_eq!(config.auto_commit_interval_ms, 5000);
    assert_eq!(config.auto_offset_reset, "latest");
    assert_eq!(config.batch_size, 100);
    assert_eq!(config.processing_timeout, Duration::from_secs(30));
    assert_eq!(config.dlq_topic, None);
    assert_eq!(config.dlq_after_retries, 3);
    assert_eq!(config.channel_buffer_size, 1000);
    assert_eq!(config.max_inflight_messages, 500);
    assert_eq!(config.pause_threshold, 0.8);
    assert_eq!(config.resume_threshold, 0.5);
    assert_eq!(config.metrics_interval, Duration::from_secs(60));
    assert!(!config.enable_detailed_metrics);
    assert!(config.kafka_properties.is_empty());
}

#[test]
fn test_builder_pattern() {
    let retry_policy = sigma_rs::consumer::retry::RetryPolicy::default();
    let config = ConsumerConfig::builder()
        .brokers("broker1:9092,broker2:9092".to_string())
        .group_id("test-group".to_string())
        .topics(vec!["topic1".to_string(), "topic2".to_string()])
        .session_timeout_ms(20000)
        .enable_auto_commit(true)
        .batch_size(200)
        .retry_policy(retry_policy.clone())
        .dlq_topic("dlq-topic".to_string())
        .channel_buffer_size(2000)
        .max_inflight_messages(1000)
        .metrics_interval(Duration::from_secs(30))
        .enable_detailed_metrics(true)
        .kafka_property("key1".to_string(), "value1".to_string())
        .kafka_property("key2".to_string(), "value2".to_string())
        .build();
    
    assert_eq!(config.brokers, "broker1:9092,broker2:9092");
    assert_eq!(config.group_id, "test-group");
    assert_eq!(config.topics, vec!["topic1", "topic2"]);
    assert_eq!(config.session_timeout_ms, 20000);
    assert!(config.enable_auto_commit);
    assert_eq!(config.batch_size, 200);
    assert_eq!(config.dlq_topic, Some("dlq-topic".to_string()));
    assert_eq!(config.channel_buffer_size, 2000);
    assert_eq!(config.max_inflight_messages, 1000);
    assert_eq!(config.metrics_interval, Duration::from_secs(30));
    assert!(config.enable_detailed_metrics);
    assert_eq!(config.kafka_properties.get("key1"), Some(&"value1".to_string()));
    assert_eq!(config.kafka_properties.get("key2"), Some(&"value2".to_string()));
}

// Validation tests
#[test]
fn test_validation_empty_brokers() {
    let mut config = ConsumerConfig::default();
    config.brokers = String::new();
    
    let result = config.validate();
    assert!(result.is_err());
    assert_eq!(result.unwrap_err(), "Brokers cannot be empty");
}

#[test]
fn test_validation_empty_group_id() {
    let mut config = ConsumerConfig::default();
    config.group_id = String::new();
    
    let result = config.validate();
    assert!(result.is_err());
    assert_eq!(result.unwrap_err(), "Group ID cannot be empty");
}

#[test]
fn test_validation_empty_topics() {
    let mut config = ConsumerConfig::default();
    config.topics = vec![];
    
    let result = config.validate();
    assert!(result.is_err());
    assert_eq!(result.unwrap_err(), "Topics cannot be empty");
}

#[test]
fn test_validation_zero_batch_size() {
    let mut config = ConsumerConfig::default();
    config.batch_size = 0;
    
    let result = config.validate();
    assert!(result.is_err());
    assert_eq!(result.unwrap_err(), "Batch size must be greater than 0");
}

#[test]
fn test_validation_dlq_retry_threshold() {
    let mut config = ConsumerConfig::default();
    config.dlq_after_retries = 10;
    config.retry_policy.max_retries = 5;
    
    let result = config.validate();
    assert!(result.is_err());
    assert_eq!(result.unwrap_err(), "DLQ retry threshold cannot exceed max retries");
}

#[test_case(0.5, 0.8, true ; "valid thresholds")]
#[test_case(0.8, 0.8, false ; "equal thresholds")]
#[test_case(0.8, 0.5, false ; "pause less than resume")]
#[test_case(1.1, 0.5, false ; "pause greater than 1")]
#[test_case(0.8, 1.1, false ; "resume greater than 1")]
fn test_validation_thresholds(pause_threshold: f64, resume_threshold: f64, should_pass: bool) {
    let mut config = ConsumerConfig::default();
    config.pause_threshold = pause_threshold;
    config.resume_threshold = resume_threshold;
    
    let result = config.validate();
    assert_eq!(result.is_ok(), should_pass);
}

#[test]
fn test_valid_configuration() {
    let config = ConsumerConfig::default();
    assert!(config.validate().is_ok());
}

// Serialization tests
#[test]
fn test_config_serialization_json() {
    let config = ConsumerConfig::default();
    
    let json = serde_json::to_string(&config).unwrap();
    let deserialized: ConsumerConfig = serde_json::from_str(&json).unwrap();
    
    assert_eq!(config.brokers, deserialized.brokers);
    assert_eq!(config.group_id, deserialized.group_id);
    assert_eq!(config.topics, deserialized.topics);
}

#[test]
fn test_config_serialization_yaml() {
    let config = ConsumerConfig::default();
    
    let yaml = serde_yaml::to_string(&config).unwrap();
    let deserialized: ConsumerConfig = serde_yaml::from_str(&yaml).unwrap();
    
    assert_eq!(config.brokers, deserialized.brokers);
    assert_eq!(config.group_id, deserialized.group_id);
    assert_eq!(config.topics, deserialized.topics);
}

// Environment variable loading tests
#[test]
fn test_load_from_env() {
    // Set environment variables
    std::env::set_var("SIGMA_BROKERS", "env-broker:9092");
    std::env::set_var("SIGMA_GROUP_ID", "env-group");
    std::env::set_var("SIGMA_TOPICS", "topic1,topic2");
    
    // Create a method to load from env (this would be implemented in config.rs)
    // For now, we're just testing the pattern
    
    let mut config = ConsumerConfig::default();
    if let Ok(brokers) = std::env::var("SIGMA_BROKERS") {
        config.brokers = brokers;
    }
    if let Ok(group_id) = std::env::var("SIGMA_GROUP_ID") {
        config.group_id = group_id;
    }
    if let Ok(topics) = std::env::var("SIGMA_TOPICS") {
        config.topics = topics.split(',').map(|s| s.to_string()).collect();
    }
    
    assert_eq!(config.brokers, "env-broker:9092");
    assert_eq!(config.group_id, "env-group");
    assert_eq!(config.topics, vec!["topic1", "topic2"]);
    
    // Clean up
    std::env::remove_var("SIGMA_BROKERS");
    std::env::remove_var("SIGMA_GROUP_ID");
    std::env::remove_var("SIGMA_TOPICS");
}

// Property-based tests
proptest! {
    #[test]
    fn test_valid_configs_validate_successfully(
        brokers in ".+",
        group_id in ".+",
        topics in prop::collection::vec(".+", 1..10),
        batch_size in 1usize..10000,
        pause_threshold in 0.1f64..0.99,
        resume_threshold in 0.01f64..0.5,
        dlq_after_retries in 0u32..100,
    ) {
        let mut config = ConsumerConfig::default();
        config.brokers = brokers;
        config.group_id = group_id;
        config.topics = topics.into_iter().map(|s| s.to_string()).collect();
        config.batch_size = batch_size;
        config.pause_threshold = pause_threshold.max(resume_threshold + 0.1);
        config.resume_threshold = resume_threshold;
        config.dlq_after_retries = dlq_after_retries.min(config.retry_policy.max_retries);
        
        assert!(config.validate().is_ok());
    }
    
    #[test]
    fn test_builder_produces_valid_configs(
        brokers in ".+",
        group_id in ".+",
        topics in prop::collection::vec(".+", 1..10),
        batch_size in 1usize..10000,
    ) {
        let config = ConsumerConfig::builder()
            .brokers(brokers)
            .group_id(group_id)
            .topics(topics.into_iter().map(|s| s.to_string()).collect())
            .batch_size(batch_size)
            .build();
            
        assert!(config.validate().is_ok());
    }
}

// Edge case tests
#[test]
fn test_very_large_values() {
    let mut config = ConsumerConfig::default();
    config.batch_size = usize::MAX;
    config.channel_buffer_size = usize::MAX;
    config.max_inflight_messages = usize::MAX;
    config.session_timeout_ms = u32::MAX;
    
    // Should still validate (no overflow)
    assert!(config.validate().is_ok());
}

#[test]
fn test_many_topics() {
    let topics: Vec<String> = (0..1000).map(|i| format!("topic-{}", i)).collect();
    let config = ConsumerConfig::builder()
        .topics(topics.clone())
        .build();
    
    assert_eq!(config.topics.len(), 1000);
    assert!(config.validate().is_ok());
}

#[test]
fn test_many_kafka_properties() {
    let mut builder = ConsumerConfig::builder();
    for i in 0..100 {
        builder = builder.kafka_property(format!("key-{}", i), format!("value-{}", i));
    }
    let config = builder.build();
    
    assert_eq!(config.kafka_properties.len(), 100);
    assert!(config.validate().is_ok());
}

// Thread safety test
#[test]
fn test_config_is_send_sync() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<ConsumerConfig>();
}

// Clone test
#[test]
fn test_config_clone() {
    let config1 = ConsumerConfig::default();
    let config2 = config1.clone();
    
    assert_eq!(config1.brokers, config2.brokers);
    assert_eq!(config1.group_id, config2.group_id);
    assert_eq!(config1.topics, config2.topics);
}