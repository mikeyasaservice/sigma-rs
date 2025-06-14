//! Main Sigma engine binary

use sigma_rs::{KafkaConfig, SigmaEngineBuilder};
use std::collections::HashMap;
use tracing::info;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing
    sigma_rs::init_tracing();
    info!("Starting Sigma Engine");

    // Build configuration from environment or command line
    let mut builder = SigmaEngineBuilder::new();

    // Add rule directories from environment
    if let Ok(rule_dirs) = std::env::var("SIGMA_RULE_DIRS") {
        for dir in rule_dirs.split(':') {
            builder = builder.add_rule_dir(dir);
        }
    }

    // Configure Kafka if environment variables are set
    if let Ok(brokers) = std::env::var("KAFKA_BROKERS") {
        let kafka_config = KafkaConfig {
            brokers,
            group_id: std::env::var("KAFKA_GROUP_ID")
                .unwrap_or_else(|_| "sigma-engine".to_string()),
            topics: std::env::var("KAFKA_TOPICS")
                .unwrap_or_else(|_| "events".to_string())
                .split(',')
                .map(|s| s.to_string())
                .collect(),
            properties: HashMap::new(),
            batch_size: None,
            max_retries: None,
            dlq_topic: None,
            backpressure_buffer_size: None,
            enable_metrics: true,
        };
        builder = builder.with_kafka(kafka_config);
    }

    // Build and run the service
    let service = builder.build().await?;
    service.run().await?;

    Ok(())
}
