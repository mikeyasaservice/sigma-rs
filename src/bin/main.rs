use clap::{Parser, ValueEnum};
use sigma_rs::{RuleSet, DynamicEvent};
use std::path::PathBuf;
use tracing_subscriber;
use serde_json::Value;
use std::io::{self, BufRead, Write};
use serde::{Deserialize, Serialize};
use std::fs;

#[derive(Debug, Clone, ValueEnum)]
enum InputSource {
    Stdin,
    #[cfg(feature = "kafka")]
    Kafka,
}

#[derive(Debug, Clone, ValueEnum)]
enum OutputTarget {
    Stdout,
    #[cfg(feature = "kafka")]
    Kafka,
}

#[derive(Parser)]
#[command(name = "sigma-rs")]
#[command(about = "High-performance Sigma rule engine", long_about = None)]
struct Cli {
    /// Path to rules directory
    #[arg(short, long)]
    rules: PathBuf,
    
    /// Input source
    #[arg(short, long, default_value = "stdin")]
    input: InputSource,
    
    /// Output target
    #[arg(short, long, default_value = "stdout")]
    output: OutputTarget,
    
    /// Configuration file (required for Kafka)
    #[arg(short, long)]
    config: Option<PathBuf>,
    
    /// Enable debug logging
    #[arg(short, long)]
    debug: bool,
}

#[derive(Debug, Default, Deserialize, Serialize)]
struct Config {
    #[serde(default)]
    kafka: KafkaConfig,
}

#[derive(Debug, Deserialize, Serialize)]
struct KafkaConfig {
    brokers: String,
    input_topic: String,
    output_topic: String,
    group_id: String,
    #[serde(default)]
    auto_offset_reset: String,
}

impl Default for KafkaConfig {
    fn default() -> Self {
        Self {
            brokers: "localhost:9092".to_string(),
            input_topic: "sigma-events".to_string(),
            output_topic: "sigma-matches".to_string(),
            group_id: "sigma-rs".to_string(),
            auto_offset_reset: "latest".to_string(),
        }
    }
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
    
    // Load configuration if needed
    #[allow(unused_variables)]
    let config = if needs_config(&cli) {
        match &cli.config {
            Some(path) => {
                let contents = fs::read_to_string(path)?;
                toml::from_str::<Config>(&contents)?
            }
            None => {
                eprintln!("Error: Configuration file required for Kafka input/output");
                eprintln!("Use --config <file> to specify configuration");
                std::process::exit(1);
            }
        }
    } else {
        Config::default()
    };
    
    // Validate rules directory exists
    if !cli.rules.exists() {
        eprintln!("Error: Rules directory not found: {}", cli.rules.display());
        eprintln!("Please specify a valid rules directory with --rules");
        std::process::exit(1);
    }
    
    if !cli.rules.is_dir() {
        eprintln!("Error: Rules path is not a directory: {}", cli.rules.display());
        std::process::exit(1);
    }
    
    // Load rules
    let mut ruleset = RuleSet::new();
    ruleset.load_directory(&cli.rules.to_string_lossy()).await?;
    
    if ruleset.len() == 0 {
        eprintln!("Error: No rules found in directory: {}", cli.rules.display());
        eprintln!("Please ensure the directory contains valid .yml rule files");
        std::process::exit(1);
    }
    
    eprintln!("Loaded {} rules from {}", ruleset.len(), cli.rules.display());
    
    // Process events based on input/output configuration
    match (cli.input, cli.output) {
        (InputSource::Stdin, OutputTarget::Stdout) => {
            process_stdin_to_stdout(ruleset).await?;
        }
        #[cfg(feature = "kafka")]
        (InputSource::Kafka, OutputTarget::Stdout) => {
            process_kafka_to_stdout(ruleset, config.kafka).await?;
        }
        #[cfg(feature = "kafka")]
        (InputSource::Stdin, OutputTarget::Kafka) => {
            process_stdin_to_kafka(ruleset, config.kafka).await?;
        }
        #[cfg(feature = "kafka")]
        (InputSource::Kafka, OutputTarget::Kafka) => {
            process_kafka_to_kafka(ruleset, config.kafka).await?;
        }
        #[cfg(feature = "kafka")]
        _ => {
            eprintln!("Invalid input/output combination");
            std::process::exit(1);
        }
    }
    
    Ok(())
}

fn needs_config(cli: &Cli) -> bool {
    #[cfg(feature = "kafka")]
    {
        matches!(cli.input, InputSource::Kafka) || matches!(cli.output, OutputTarget::Kafka)
    }
    #[cfg(not(feature = "kafka"))]
    {
        let _ = cli; // Suppress unused warning
        false
    }
}

async fn process_stdin_to_stdout(ruleset: RuleSet) -> Result<(), Box<dyn std::error::Error>> {
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut stdout_lock = stdout.lock();
    
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
                    "timestamp": chrono::Utc::now().to_rfc3339(),
                    "event": event,
                    "rule_id": rule_match.rule_id,
                    "rule_title": rule_match.rule_title,
                });
                writeln!(stdout_lock, "{}", serde_json::to_string(&output)?)?;
            }
        }
    }
    
    Ok(())
}

#[cfg(feature = "kafka")]
async fn process_kafka_to_stdout(
    ruleset: RuleSet,
    config: KafkaConfig,
) -> Result<(), Box<dyn std::error::Error>> {
    use rdkafka::consumer::{Consumer, StreamConsumer};
    use rdkafka::config::ClientConfig;
    use rdkafka::message::Message;
    use futures::stream::StreamExt;
    
    let consumer: StreamConsumer = ClientConfig::new()
        .set("bootstrap.servers", &config.brokers)
        .set("group.id", &config.group_id)
        .set("auto.offset.reset", &config.auto_offset_reset)
        .set("enable.auto.commit", "false")
        .create()?;
    
    consumer.subscribe(&[&config.input_topic])?;
    
    let stdout = io::stdout();
    let mut stdout_lock = stdout.lock();
    
    let mut message_stream = consumer.stream();
    
    while let Some(message) = message_stream.next().await {
        match message {
            Ok(msg) => {
                if let Some(payload) = msg.payload() {
                    if let Ok(data) = std::str::from_utf8(payload) {
                        if let Ok(event) = serde_json::from_str::<Value>(data) {
                            let dynamic_event = DynamicEvent::new(event.clone());
                            let result = ruleset.evaluate(&dynamic_event).await?;
                            
                            for rule_match in &result.matches {
                                if rule_match.matched {
                                    let output = serde_json::json!({
                                        "timestamp": chrono::Utc::now().to_rfc3339(),
                                        "event": event,
                                        "rule_id": rule_match.rule_id,
                                        "rule_title": rule_match.rule_title,
                                    });
                                    writeln!(stdout_lock, "{}", serde_json::to_string(&output)?)?;
                                }
                            }
                        }
                    }
                }
                consumer.store_offset_from_message(&msg)?;
            }
            Err(e) => {
                eprintln!("Kafka error: {}", e);
            }
        }
    }
    
    Ok(())
}

#[cfg(feature = "kafka")]
async fn process_stdin_to_kafka(
    ruleset: RuleSet,
    config: KafkaConfig,
) -> Result<(), Box<dyn std::error::Error>> {
    use rdkafka::producer::{FutureProducer, FutureRecord};
    use rdkafka::config::ClientConfig;
    
    let producer: FutureProducer = ClientConfig::new()
        .set("bootstrap.servers", &config.brokers)
        .set("message.timeout.ms", "5000")
        .create()?;
    
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
                    "timestamp": chrono::Utc::now().to_rfc3339(),
                    "event": event,
                    "rule_id": rule_match.rule_id,
                    "rule_title": rule_match.rule_title,
                });
                
                let payload = serde_json::to_string(&output)?;
                let record = FutureRecord::to(&config.output_topic)
                    .payload(&payload)
                    .key(&rule_match.rule_id);
                
                producer.send(record, std::time::Duration::from_secs(0)).await
                    .map_err(|(e, _)| e)?;
            }
        }
    }
    
    Ok(())
}

#[cfg(feature = "kafka")]
async fn process_kafka_to_kafka(
    ruleset: RuleSet,
    config: KafkaConfig,
) -> Result<(), Box<dyn std::error::Error>> {
    use rdkafka::consumer::{Consumer, StreamConsumer};
    use rdkafka::producer::{FutureProducer, FutureRecord};
    use rdkafka::config::ClientConfig;
    use rdkafka::message::Message;
    use futures::stream::StreamExt;
    
    let consumer: StreamConsumer = ClientConfig::new()
        .set("bootstrap.servers", &config.brokers)
        .set("group.id", &config.group_id)
        .set("auto.offset.reset", &config.auto_offset_reset)
        .set("enable.auto.commit", "false")
        .create()?;
    
    let producer: FutureProducer = ClientConfig::new()
        .set("bootstrap.servers", &config.brokers)
        .set("message.timeout.ms", "5000")
        .create()?;
    
    consumer.subscribe(&[&config.input_topic])?;
    
    let mut message_stream = consumer.stream();
    
    while let Some(message) = message_stream.next().await {
        match message {
            Ok(msg) => {
                if let Some(payload) = msg.payload() {
                    if let Ok(data) = std::str::from_utf8(payload) {
                        if let Ok(event) = serde_json::from_str::<Value>(data) {
                            let dynamic_event = DynamicEvent::new(event.clone());
                            let result = ruleset.evaluate(&dynamic_event).await?;
                            
                            for rule_match in &result.matches {
                                if rule_match.matched {
                                    let output = serde_json::json!({
                                        "timestamp": chrono::Utc::now().to_rfc3339(),
                                        "event": event,
                                        "rule_id": rule_match.rule_id,
                                        "rule_title": rule_match.rule_title,
                                    });
                                    
                                    let payload = serde_json::to_string(&output)?;
                                    let record = FutureRecord::to(&config.output_topic)
                                        .payload(&payload)
                                        .key(&rule_match.rule_id);
                                    
                                    producer.send(record, std::time::Duration::from_secs(0)).await
                                        .map_err(|(e, _)| e)?;
                                }
                            }
                        }
                    }
                }
                consumer.store_offset_from_message(&msg)?;
            }
            Err(e) => {
                eprintln!("Kafka error: {}", e);
            }
        }
    }
    
    Ok(())
}