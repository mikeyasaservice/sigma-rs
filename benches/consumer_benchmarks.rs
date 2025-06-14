use async_trait::async_trait;
use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use rdkafka::{message::OwnedMessage, producer::FutureProducer, ClientConfig, Timestamp};
use sigma_rs::consumer::{
    backpressure::BackpressureController,
    dlq::DlqProducer,
    metrics::ConsumerMetrics,
    offset_manager::OffsetManager,
    processor::MessageProcessor,
    retry::{RetryExecutor, RetryPolicy},
};
use std::hint::black_box;
use std::sync::Arc;
use std::time::Duration;
use tokio::runtime::Runtime;

// Simple processor for benchmarking
struct BenchProcessor;

#[async_trait]
impl MessageProcessor for BenchProcessor {
    type Error = String;

    async fn process(&self, _message: &OwnedMessage) -> Result<(), Self::Error> {
        // Minimal processing
        Ok(())
    }

    async fn on_success(&self, _message: &OwnedMessage) {}
    async fn on_failure(&self, _error: &Self::Error, _message: &OwnedMessage) {}
}

// Helper to create test messages
fn create_test_message(size: usize) -> OwnedMessage {
    let payload = vec![0u8; size];
    OwnedMessage::new(
        Some(b"test-key".to_vec()),
        Some(payload),
        "test-topic".to_string(),
        Timestamp::CreateTime(0),
        0,
        42,
        None,
    )
}

// Benchmark message processing
fn benchmark_message_processing(c: &mut Criterion) {
    let runtime = Runtime::new().unwrap();
    let processor = BenchProcessor;

    let mut group = c.benchmark_group("message_processing");

    for size in [100, 1024, 10240, 102400].iter() {
        group.throughput(Throughput::Bytes(*size as u64));
        group.bench_with_input(
            BenchmarkId::new("process_message", size),
            size,
            |b, &size| {
                let message = create_test_message(size);
                b.to_async(&runtime)
                    .iter(|| async { processor.process(black_box(&message)).await.unwrap() });
            },
        );
    }

    group.finish();
}

// Benchmark backpressure controller
fn benchmark_backpressure(c: &mut Criterion) {
    let runtime = Runtime::new().unwrap();

    let mut group = c.benchmark_group("backpressure");

    group.bench_function("acquire_permit", |b| {
        let controller = BackpressureController::new(1000, 0.8, 0.5);
        b.to_async(&runtime).iter(|| async {
            let _permit = controller.acquire().await;
        });
    });

    group.bench_function("check_pause_resume", |b| {
        let controller = BackpressureController::new(100, 0.8, 0.5);
        b.iter(|| {
            black_box(controller.should_pause());
            black_box(controller.should_resume());
        });
    });

    group.bench_function("update_memory", |b| {
        let controller = BackpressureController::with_memory_limit(100, 1024 * 1024, 0.8, 0.5);
        b.iter(|| {
            controller.update_avg_message_size(black_box(1024));
        });
    });

    group.finish();
}

// Benchmark metrics collection
fn benchmark_metrics(c: &mut Criterion) {
    let mut group = c.benchmark_group("metrics");

    group.bench_function("increment_counter", |b| {
        let metrics = ConsumerMetrics::new();
        b.iter(|| {
            metrics.increment_consumed();
        });
    });

    group.bench_function("record_duration", |b| {
        let metrics = ConsumerMetrics::new();
        let duration = Duration::from_millis(10);
        b.iter(|| {
            metrics.record_processing_duration(black_box(duration));
        });
    });

    group.bench_function("record_partition_lag", |b| {
        let metrics = ConsumerMetrics::new();
        b.iter(|| {
            metrics.record_partition_lag(
                black_box("topic".to_string()),
                black_box(0),
                black_box(100),
            );
        });
    });

    group.bench_function("export_prometheus", |b| {
        let metrics = ConsumerMetrics::new();
        // Set up some data
        for _ in 0..100 {
            metrics.increment_consumed();
            metrics.increment_processed();
        }

        b.iter(|| {
            black_box(metrics.export_prometheus());
        });
    });

    group.finish();
}

// Benchmark offset management
fn benchmark_offset_manager(c: &mut Criterion) {
    let runtime = Runtime::new().unwrap();

    let mut group = c.benchmark_group("offset_manager");

    group.bench_function("mark_offset", |b| {
        let manager = OffsetManager::new(100, Duration::from_secs(5));
        b.to_async(&runtime).iter(|| async {
            manager
                .mark_offset(black_box("topic".to_string()), black_box(0), black_box(42))
                .await;
        });
    });

    group.bench_function("bulk_mark_offsets", |b| {
        let manager = OffsetManager::new(1000, Duration::from_secs(5));
        b.to_async(&runtime).iter(|| async {
            for i in 0..100 {
                manager
                    .mark_offset("topic".to_string(), i % 10, i as i64)
                    .await;
            }
        });
    });

    group.finish();
}

// Benchmark retry execution
fn benchmark_retry(c: &mut Criterion) {
    let runtime = Runtime::new().unwrap();

    let mut group = c.benchmark_group("retry");

    let policy = RetryPolicy {
        max_retries: 3,
        initial_delay: Duration::from_millis(100),
        max_delay: Duration::from_secs(1),
        exponential_backoff: true,
        jitter: true,
    };

    group.bench_function("successful_operation", |b| {
        let executor = RetryExecutor::new(policy.clone());
        b.to_async(&runtime).iter(|| async {
            executor
                .execute(|| async { Ok::<_, String>("success") })
                .await
        });
    });

    group.bench_function("retry_with_failures", |b| {
        let executor = RetryExecutor::new(policy.clone());
        b.to_async(&runtime).iter(|| async {
            let mut attempt = 0;
            executor
                .execute(|| async {
                    attempt += 1;
                    if attempt < 3 {
                        Err("temporary error".to_string())
                    } else {
                        Ok("success")
                    }
                })
                .await
        });
    });

    group.finish();
}

// Benchmark DLQ operations
fn benchmark_dlq(c: &mut Criterion) {
    // Skip if we can't create a producer (requires Kafka)
    if let Ok(producer) = ClientConfig::new()
        .set("bootstrap.servers", "localhost:9092")
        .create::<FutureProducer>()
    {
        let runtime = Runtime::new().unwrap();
        let dlq = Arc::new(DlqProducer::new(producer, "dlq-topic".to_string()));

        let mut group = c.benchmark_group("dlq");

        group.bench_function("prepare_dlq_message", |b| {
            let message = create_test_message(1024);
            b.iter(|| {
                // Just benchmark the preparation, not actual sending
                let _ = black_box(&message);
                let _ = black_box("error message");
                let _ = black_box(1u32);
            });
        });

        group.finish();
    }
}

// Benchmark concurrent operations
fn benchmark_concurrency(c: &mut Criterion) {
    let runtime = Runtime::new().unwrap();

    let mut group = c.benchmark_group("concurrency");

    group.bench_function("concurrent_metrics_updates", |b| {
        let metrics = Arc::new(ConsumerMetrics::new());
        b.to_async(&runtime).iter(|| async {
            let mut handles = vec![];

            for _ in 0..10 {
                let m = metrics.clone();
                let handle = tokio::spawn(async move {
                    for _ in 0..100 {
                        m.increment_consumed();
                    }
                });
                handles.push(handle);
            }

            for handle in handles {
                handle.await.unwrap();
            }
        });
    });

    group.bench_function("concurrent_backpressure", |b| {
        let controller = Arc::new(BackpressureController::new(100, 0.8, 0.5));
        b.to_async(&runtime).iter(|| async {
            let mut handles = vec![];

            for _ in 0..10 {
                let ctrl = controller.clone();
                let handle = tokio::spawn(async move {
                    let _permit = ctrl.acquire().await;
                });
                handles.push(handle);
            }

            for handle in handles {
                handle.await.unwrap();
            }
        });
    });

    group.finish();
}

// Benchmark end-to-end scenarios
fn benchmark_scenarios(c: &mut Criterion) {
    let runtime = Runtime::new().unwrap();

    let mut group = c.benchmark_group("scenarios");

    // Simulate processing a batch of messages
    group.bench_function("batch_processing", |b| {
        let processor = BenchProcessor;
        let metrics = ConsumerMetrics::new();
        let controller = BackpressureController::new(1000, 0.8, 0.5);

        let messages: Vec<_> = (0..100).map(|_| create_test_message(1024)).collect();

        b.to_async(&runtime).iter(|| async {
            for message in &messages {
                let _permit = controller.acquire().await;

                metrics.increment_consumed();
                let start = std::time::Instant::now();

                if processor.process(message).await.is_ok() {
                    metrics.increment_processed();
                } else {
                    metrics.increment_failed();
                }

                metrics.record_processing_duration(start.elapsed());
            }
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    benchmark_message_processing,
    benchmark_backpressure,
    benchmark_metrics,
    benchmark_offset_manager,
    benchmark_retry,
    benchmark_dlq,
    benchmark_concurrency,
    benchmark_scenarios
);

criterion_main!(benches);
