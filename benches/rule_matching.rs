//! Performance benchmarks for rule matching

use criterion::{criterion_group, criterion_main, Criterion};
use sigma_rs::{DynamicEvent, Event};
use std::hint::black_box;



fn benchmark_event_creation(c: &mut Criterion) {
    c.bench_function("event creation", |b| {
        b.iter(|| {
            let data = serde_json::json!({
                "message": "test message",
                "field1": "value1",
                "nested": {
                    "field2": "value2"
                }
            });
            black_box(DynamicEvent::new(data));
        });
    });
}

fn benchmark_event_selection(c: &mut Criterion) {
    let data = serde_json::json!({
        "message": "test message",
        "field1": "value1",
        "nested": {
            "nested": {
                "field2": "value2"
            }
        }
    });
    let event = DynamicEvent::new(data);
    
    c.bench_function("event selection", |b| {
            black_box(event.select("nested.field2"));
        });
}

criterion_group!(benches, benchmark_event_creation, benchmark_event_selection);
criterion_main!(benches);
