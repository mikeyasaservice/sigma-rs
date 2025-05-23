//! Validation tests for string matching optimizations
//! 
//! This test validates that our optimizations maintain correctness
//! while improving performance.

use sigma_rs::pattern::{
    string_matcher::{ContentPattern, PrefixPattern, SuffixPattern},
    traits::StringMatcher,
};

#[test]
fn test_content_pattern_case_sensitive_optimization() {
    let pattern = ContentPattern {
        token: "process_creation".to_string(),
        lowercase: false,
        no_collapse_ws: false,
    };

    // Test exact matches
    assert!(pattern.string_match("process_creation"));
    assert!(!pattern.string_match("Process_Creation"));
    assert!(!pattern.string_match("process_creation_event"));
    assert!(!pattern.string_match(""));
}

#[test]
fn test_content_pattern_case_insensitive_optimization() {
    let pattern = ContentPattern {
        token: "Process_Creation".to_string(),
        lowercase: true,
        no_collapse_ws: false,
    };

    // Test case insensitive matches using optimized eq_ignore_ascii_case
    assert!(pattern.string_match("process_creation"));
    assert!(pattern.string_match("Process_Creation"));
    assert!(pattern.string_match("PROCESS_CREATION"));
    assert!(pattern.string_match("pRoCeSs_CrEaTiOn"));
    assert!(!pattern.string_match("process_creation_event"));
    assert!(!pattern.string_match(""));
}

#[test]
fn test_prefix_pattern_optimization() {
    let pattern = PrefixPattern {
        token: "process".to_string(),
        lowercase: false,
        no_collapse_ws: false,
    };

    // Test prefix matches without allocation
    assert!(pattern.string_match("process"));
    assert!(pattern.string_match("process_creation"));
    assert!(pattern.string_match("processes"));
    assert!(!pattern.string_match("Process"));
    assert!(!pattern.string_match("subprocess"));
    assert!(!pattern.string_match(""));
}

#[test]
fn test_prefix_pattern_case_insensitive_optimization() {
    let pattern = PrefixPattern {
        token: "Process".to_string(),
        lowercase: true,
        no_collapse_ws: false,
    };

    // Test case insensitive prefix without allocation
    assert!(pattern.string_match("process"));
    assert!(pattern.string_match("Process"));
    assert!(pattern.string_match("PROCESS"));
    assert!(pattern.string_match("process_creation"));
    assert!(pattern.string_match("PROCESS_CREATION"));
    assert!(!pattern.string_match("subprocess"));
    assert!(!pattern.string_match(""));
}

#[test]
fn test_suffix_pattern_optimization() {
    let pattern = SuffixPattern {
        token: "creation".to_string(),
        lowercase: false,
        no_collapse_ws: false,
    };

    // Test suffix matches without allocation
    assert!(pattern.string_match("creation"));
    assert!(pattern.string_match("process_creation"));
    assert!(pattern.string_match("file_creation"));
    assert!(!pattern.string_match("Creation"));
    assert!(!pattern.string_match("creation_event"));
    assert!(!pattern.string_match(""));
}

#[test]
fn test_suffix_pattern_case_insensitive_optimization() {
    let pattern = SuffixPattern {
        token: "Creation".to_string(),
        lowercase: true,
        no_collapse_ws: false,
    };

    // Test case insensitive suffix without allocation
    assert!(pattern.string_match("creation"));
    assert!(pattern.string_match("Creation"));
    assert!(pattern.string_match("CREATION"));
    assert!(pattern.string_match("process_creation"));
    assert!(pattern.string_match("PROCESS_CREATION"));
    assert!(!pattern.string_match("creation_event"));
    assert!(!pattern.string_match(""));
}

#[test]
fn test_edge_cases_optimization() {
    // Test empty token
    let empty_pattern = ContentPattern {
        token: "".to_string(),
        lowercase: false,
        no_collapse_ws: false,
    };
    assert!(empty_pattern.string_match(""));
    assert!(!empty_pattern.string_match("anything"));

    // Test unicode handling
    let unicode_pattern = ContentPattern {
        token: "测试".to_string(),
        lowercase: false,
        no_collapse_ws: false,
    };
    assert!(unicode_pattern.string_match("测试"));
    assert!(!unicode_pattern.string_match("测试文档"));

    // Test very short strings
    let short_pattern = PrefixPattern {
        token: "a".to_string(),
        lowercase: false,
        no_collapse_ws: false,
    };
    assert!(short_pattern.string_match("a"));
    assert!(short_pattern.string_match("abc"));
    assert!(!short_pattern.string_match(""));
    assert!(!short_pattern.string_match("ba"));
}

#[test] 
fn test_performance_critical_patterns() {
    // Test patterns commonly used in security rules
    let patterns = vec![
        ("powershell", "powershell.exe -Command Get-Process"),
        ("cmd", "cmd.exe /c dir"),
        ("sysmon", "sysmon_event_id_1_process_creation"),
        ("EventID", "EventID=1"),
    ];

    for (token, test_str) in patterns {
        let case_sensitive = ContentPattern {
            token: token.to_string(),
            lowercase: false,
            no_collapse_ws: false,
        };
        
        let case_insensitive = ContentPattern {
            token: token.to_string(),
            lowercase: true,
            no_collapse_ws: false,
        };

        // Verify both optimized paths work
        assert!(case_insensitive.string_match(test_str));
        if token.chars().all(|c| c.is_ascii_lowercase()) {
            // Only test case sensitive if token is lowercase
            assert!(case_sensitive.string_match(test_str));
        }
    }
}

#[test]
fn test_boundary_conditions() {
    // Test exact length matches for prefix
    let prefix_pattern = PrefixPattern {
        token: "test".to_string(),
        lowercase: false,
        no_collapse_ws: false,
    };
    assert!(prefix_pattern.string_match("test"));  // Exact length match
    assert!(prefix_pattern.string_match("test1")); // Length + 1
    assert!(!prefix_pattern.string_match("tes"));  // Length - 1

    // Test exact length matches for suffix  
    let suffix_pattern = SuffixPattern {
        token: "test".to_string(),
        lowercase: false,
        no_collapse_ws: false,
    };
    assert!(suffix_pattern.string_match("test"));  // Exact length match
    assert!(suffix_pattern.string_match("1test")); // Length + 1
    assert!(!suffix_pattern.string_match("tes"));  // Length - 1
}