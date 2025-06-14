//! Benchmark for string matching performance optimizations
//! 
//! This benchmark validates the performance improvements made to reduce
//! string allocations in hot path pattern matching.

use criterion::{criterion_group, criterion_main, Criterion};
use std::hint::black_box;
use std::sync::Arc;
use sigma_rs::pattern::{
    string_matcher::{ContentPattern, PrefixPattern, SuffixPattern},
    traits::StringMatcher,
};

/// Test data for benchmarking
const TEST_STRINGS: &[&str] = &[
    "process_creation_win_malware_detection",
    "network_connection_suspicious_domain", 
    "file_access_system_directory",
    "registry_modification_autorun_key",
    "powershell_command_execution",
    "cmd_command_line_injection",
    "dns_query_malicious_domain",
    "http_request_suspicious_user_agent",
    "sysmon_event_id_1_process_creation",
    "windows_security_log_authentication_failure",
];

const PATTERN_TOKEN: &str = "process_creation";

    let pattern = ContentPattern {
        token: Arc::from(PATTERN_TOKEN),
        lowercase: false,
        no_collapse_ws: false,
    };
    };

    c.bench_function("content_pattern_case_sensitive", |b| {
        b.iter(|| {
            for test_str in TEST_STRINGS {
                black_box(pattern.string_match(black_box(test_str)));
            }
        })
    });
}

fn bench_content_pattern_case_insensitive(c: &mut Criterion) {
    let pattern = ContentPattern {
        token: PATTERN_TOKEN.to_string(),
        lowercase: true,
        no_collapse_ws: false,
    };

    c.bench_function("content_pattern_case_insensitive", |b| {
        b.iter(|| {
            for test_str in TEST_STRINGS {
                black_box(pattern.string_match(black_box(test_str)));
            }
        })
    });
}

fn bench_prefix_pattern_case_sensitive(c: &mut Criterion) {
    let pattern = PrefixPattern {
        token: PATTERN_TOKEN.to_string(),
        lowercase: false,
        no_collapse_ws: false,
    };

    c.bench_function("prefix_pattern_case_sensitive", |b| {
        b.iter(|| {
            for test_str in TEST_STRINGS {
                black_box(pattern.string_match(black_box(test_str)));
            }
        })
    });
}

fn bench_prefix_pattern_case_insensitive(c: &mut Criterion) {
    let pattern = PrefixPattern {
        token: PATTERN_TOKEN.to_string(),
        lowercase: true,
        no_collapse_ws: false,
    };

    c.bench_function("prefix_pattern_case_insensitive", |b| {
        b.iter(|| {
            for test_str in TEST_STRINGS {
                black_box(pattern.string_match(black_box(test_str)));
            }
        })
    });
}

fn bench_suffix_pattern_case_sensitive(c: &mut Criterion) {
    let pattern = SuffixPattern {
        token: "creation".to_string(),
        lowercase: false,
        no_collapse_ws: false,
    };

    c.bench_function("suffix_pattern_case_sensitive", |b| {
        b.iter(|| {
            for test_str in TEST_STRINGS {
                black_box(pattern.string_match(black_box(test_str)));
            }
        })
    });
}

fn bench_suffix_pattern_case_insensitive(c: &mut Criterion) {
    let pattern = SuffixPattern {
        token: "creation".to_string(),
        lowercase: true,
        no_collapse_ws: false,
    };

    c.bench_function("suffix_pattern_case_insensitive", |b| {
        b.iter(|| {
            for test_str in TEST_STRINGS {
                black_box(pattern.string_match(black_box(test_str)));
            }
        })
    });
}

fn bench_mixed_case_large_strings(c: &mut Criterion) {
    let large_strings = &[
        "ProcessCreation".repeat(100),
        "PROCESS_CREATION".repeat(100), 
        "process_creation".repeat(100),
        "Process_Creation_Event_Log_Entry_With_Multiple_Fields_And_Long_Command_Line_Arguments_That_May_Contain_Suspicious_Patterns_Or_Indicators_Of_Compromise".repeat(10),
    ];

    let pattern = ContentPattern {
        token: "process_creation".to_string(),
        lowercase: true,
        no_collapse_ws: false,
    };

    c.bench_function("large_strings_case_insensitive", |b| {
        b.iter(|| {
            for test_str in large_strings {
                black_box(pattern.string_match(black_box(test_str)));
            }
        })
    });
}

criterion_group!(
    benches,
    bench_content_pattern_case_sensitive,
    bench_content_pattern_case_insensitive,
    bench_prefix_pattern_case_sensitive,
    bench_prefix_pattern_case_insensitive,
    bench_suffix_pattern_case_sensitive,
    bench_suffix_pattern_case_insensitive,
    bench_mixed_case_large_strings
);

criterion_main!(benches);