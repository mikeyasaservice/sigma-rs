//! Memory optimization benchmarks
//! 
//! These benchmarks validate the memory allocation improvements made to the sigma-rs codebase,
//! specifically focusing on string allocation optimizations in hot paths.

use criterion::{criterion_group, criterion_main, Criterion};
use std::hint::black_box;
use sigma_rs::{
    pattern::{
        whitespace::handle_whitespace,
        factory::new_string_matcher,
        TextPatternModifier,
    },
    parser::Parser,
    rule::Detection,
    ast::FieldRule,
    ast::FieldPattern,
};
use std::sync::Arc;

/// Benchmark whitespace handling optimization
fn bench_whitespace_optimization(c: &mut Criterion) {
    let test_cases = vec![
        ("no_whitespace", "simple_text"),
        ("single_spaces", "text with spaces"),
        ("multiple_spaces", "text  with   multiple    spaces"),
        ("mixed_whitespace", "text\t\nwith\r\nvarious\x20whitespace"),
    ];

    c.bench_function("whitespace_handling_old_style", |b| {
        b.iter(|| {
            for (_, input) in &test_cases {
                // Simulate old behavior that always allocates
                let _result = input.replace(regex::Regex::new(r"\s+").unwrap().as_str(), " ");
                black_box(_result);
            }
        })
    });

    c.bench_function("whitespace_handling_optimized", |b| {
        b.iter(|| {
            for (_, input) in &test_cases {
                let result = handle_whitespace(input, false);
                black_box(result);
            }
        })
    });

    c.bench_function("whitespace_handling_no_collapse", |b| {
        b.iter(|| {
            for (_, input) in &test_cases {
                let result = handle_whitespace(input, true);
                black_box(result);
            }
        })
    });
}

/// Benchmark parser field name optimization
fn bench_parser_field_optimization(c: &mut Criterion) {
    // Create a test detection with condition
    let mut detection = Detection::new();
    detection.insert("condition".to_string(), serde_json::json!("EventID = 1 and ProcessName = 'test.exe'"));

    c.bench_function("parser_field_allocation", |b| {
        b.iter(|| {
            // Create multiple field rules to test Arc<str> vs String allocation
            let field_rules: Vec<FieldRule> = (0..100).map(|i| {
                FieldRule::new(
                    Arc::from(format!("field_{}", i)),
                    FieldPattern::String {
                        matcher: Arc::new(sigma_rs::pattern::string_matcher::ContentPattern {
                            token: Arc::from("test"),
                            lowercase: false,
                            no_collapse_ws: false,
                        }),
                        pattern_desc: Arc::from("test"),
                    },
                )
            }).collect();
            black_box(field_rules);
        })
    });

    c.bench_function("parser_creation", |b| {
        b.iter(|| {
            let parser = Parser::new(detection.clone(), false);
            black_box(parser);
        })
    });
}

/// Benchmark pattern factory optimization
fn bench_pattern_factory_optimization(c: &mut Criterion) {
    let patterns = vec![
        "simple",
        "with*wildcard",
        "contains_pattern",
        "prefix*",
        "*suffix",
        "complex*pattern*with*multiple*wildcards",
    ];

    c.bench_function("pattern_factory_contains", |b| {
        b.iter(|| {
            for pattern in &patterns {
                let matcher = new_string_matcher(
                    TextPatternModifier::Contains,
                    false,
                    false,
                    false,
                    vec![pattern.to_string()],
                );
                black_box(matcher);
            }
        })
    });

    c.bench_function("pattern_factory_keyword", |b| {
        b.iter(|| {
            for pattern in &patterns {
                let matcher = new_string_matcher(
                    TextPatternModifier::Keyword,
                    false,
                    false,
                    false,
                    vec![pattern.to_string()],
                );
                black_box(matcher);
            }
        })
    });
}

/// Benchmark string allocation patterns that were optimized
fn bench_string_allocation_patterns(c: &mut Criterion) {
    let test_strings: Vec<String> = (0..1000).map(|i| format!("test_string_{}", i)).collect();

    c.bench_function("string_clone_pattern", |b| {
        b.iter(|| {
            let cloned: Vec<String> = test_strings.iter().map(|s| s.clone()).collect();
            black_box(cloned);
        })
    });

    c.bench_function("arc_str_pattern", |b| {
        b.iter(|| {
            let arcs: Vec<Arc<str>> = test_strings.iter().map(|s| Arc::from(s.as_str())).collect();
            black_box(arcs);
        })
    });

    c.bench_function("arc_str_sharing", |b| {
        let shared_arcs: Vec<Arc<str>> = test_strings.iter().map(|s| Arc::from(s.as_str())).collect();
        b.iter(|| {
            let clones: Vec<Arc<str>> = shared_arcs.iter().map(|arc| arc.clone()).collect();
            black_box(clones);
        })
    });
}

/// Memory pressure simulation with various allocation patterns
fn bench_memory_pressure(c: &mut Criterion) {
    c.bench_function("memory_pressure_old_pattern", |b| {
        b.iter(|| {
            let mut strings = Vec::new();
            for i in 0..1000 {
                // Simulate old allocation-heavy patterns
                let field_name = format!("field_{}", i);
                let pattern_desc = format!("pattern_{}", i);
                let condition = format!("condition_{}", i);
                
                strings.push(field_name);
                strings.push(pattern_desc);
                strings.push(condition);
            }
            black_box(strings);
        })
    });

    c.bench_function("memory_pressure_optimized_pattern", |b| {
        b.iter(|| {
            let mut arcs = Vec::new();
            for i in 0..1000 {
                // Simulate optimized patterns with Arc<str>
                let field_name: Arc<str> = Arc::from(format!("field_{}", i));
                let pattern_desc: Arc<str> = Arc::from(format!("pattern_{}", i));
                let condition: Arc<str> = Arc::from(format!("condition_{}", i));
                
                arcs.push(field_name);
                arcs.push(pattern_desc);
                arcs.push(condition);
            }
            black_box(arcs);
        })
    });
}

criterion_group!(
    benches,
    bench_whitespace_optimization,
    bench_parser_field_optimization,
    bench_pattern_factory_optimization,
    bench_string_allocation_patterns,
    bench_memory_pressure
);
criterion_main!(benches);