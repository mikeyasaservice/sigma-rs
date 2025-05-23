use criterion::{criterion_group, criterion_main, Criterion};
use std::hint::black_box;
use sigma_rs::rule::rule_from_yaml;

const SIMPLE_RULE: &str = r#"
title: Simple Rule
id: simple-rule-test
detection:
    selection:
        EventID: 4688
    condition: selection
"#;

fn benchmark_rule_parsing(c: &mut Criterion) {
    c.bench_function("parse_simple_rule", |b| {
        b.iter(|| {
            let rule = rule_from_yaml(black_box(SIMPLE_RULE.as_bytes())).unwrap();
            black_box(rule);
        });
    });
}

criterion_group!(benches, benchmark_rule_parsing);
criterion_main!(benches);