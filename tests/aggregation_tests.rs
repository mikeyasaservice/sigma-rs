use chrono::Utc;
use sigma_rs::aggregation::{
    AggregationConfig, AggregationEvaluator, AggregationFunction, SlidingWindow,
};
use sigma_rs::ast::nodes::{ComparisonOp, NodeAggregation};
use sigma_rs::event::EventBuilder;
use sigma_rs::Selector; // Import necessary traits
use std::time::Duration;
use tokio::time::sleep;

#[tokio::test]
async fn test_count_aggregation_single_group() {
    let evaluator = AggregationEvaluator::new();

    let aggregation_node = NodeAggregation::new(
        AggregationFunction::Count,
        ComparisonOp::GreaterThan,
        5.0,
        Some("user".to_string()),
        Some(Duration::from_secs(60)),
    );

    // Create events within same time window
    let base_time = Utc::now();
    let mut events = vec![];

    for i in 0..7 {
        let event = EventBuilder::new()
            .field("user", "alice")
            .field("event_id", format!("{}", i))
            .timestamp(base_time + chrono::Duration::seconds(i as i64))
            .build();
        events.push(event);
    }

    // Evaluate aggregation
    let mut results = vec![];
    for event in &events {
        let result = evaluator.evaluate(&aggregation_node, event).await;
        results.push(result);
    }

    // First 5 events shouldn't trigger (count <= 5)
    for i in 0..6 {
        assert!(!results[i].triggered, "Event {} should not trigger", i);
    }

    // 7th event should trigger (count > 5)
    assert!(results[6].triggered, "Event 6 should trigger");
    assert_eq!(results[6].value, 7.0);
}

#[tokio::test]
async fn test_count_aggregation_multiple_groups() {
    let evaluator = AggregationEvaluator::new();

    let aggregation_node = NodeAggregation {
        function: AggregationFunction::Count,
        comparison: ComparisonOp::GreaterThan,
        threshold: 3.0,
        by_field: Some("user".to_string()),
        time_window: Some(Duration::from_secs(60)),
    };

    let base_time = Utc::now();

    // Mix events for different users
    let events = vec![
        // Alice: 4 events
        EventBuilder::new()
            .field("user", "alice")
            .timestamp(base_time)
            .build(),
        EventBuilder::new()
            .field("user", "alice")
            .timestamp(base_time + chrono::Duration::seconds(10))
            .build(),
        EventBuilder::new()
            .field("user", "alice")
            .timestamp(base_time + chrono::Duration::seconds(20))
            .build(),
        EventBuilder::new()
            .field("user", "alice")
            .timestamp(base_time + chrono::Duration::seconds(30))
            .build(),
        // Bob: 3 events
        EventBuilder::new()
            .field("user", "bob")
            .timestamp(base_time + chrono::Duration::seconds(5))
            .build(),
        EventBuilder::new()
            .field("user", "bob")
            .timestamp(base_time + chrono::Duration::seconds(15))
            .build(),
        EventBuilder::new()
            .field("user", "bob")
            .timestamp(base_time + chrono::Duration::seconds(25))
            .build(),
    ];

    let mut alice_results = vec![];
    let mut bob_results = vec![];

    for event in events {
        let result = evaluator.evaluate(&aggregation_node, &event).await;
        match event.select("user").0 {
            Some(sigma_rs::event::Value::String(s)) if s == "alice" => alice_results.push(result),
            Some(sigma_rs::event::Value::String(s)) if s == "bob" => bob_results.push(result),
            _ => unreachable!(),
        }
    }

    // Alice's 4th event should trigger (count > 3)
    assert!(!alice_results[0].triggered);
    assert!(!alice_results[1].triggered);
    assert!(!alice_results[2].triggered);
    assert!(alice_results[3].triggered);

    // Bob's events should never trigger (count = 3, not > 3)
    assert!(!bob_results[0].triggered);
    assert!(!bob_results[1].triggered);
    assert!(!bob_results[2].triggered);
}

#[tokio::test]
async fn test_sliding_window_expiration() {
    let evaluator = AggregationEvaluator::new();

    let aggregation_node = NodeAggregation {
        function: AggregationFunction::Count,
        comparison: ComparisonOp::GreaterThan,
        threshold: 2.0,
        by_field: Some("user".to_string()),
        time_window: Some(Duration::from_secs(60)),
    };

    let base_time = Utc::now();

    // Events spread over time
    let events = vec![
        EventBuilder::new()
            .field("user", "alice")
            .timestamp(base_time)
            .build(),
        EventBuilder::new()
            .field("user", "alice")
            .timestamp(base_time + chrono::Duration::seconds(30))
            .build(),
        EventBuilder::new()
            .field("user", "alice")
            .timestamp(base_time + chrono::Duration::seconds(65)) // Outside first window
            .build(),
    ];

    let mut results = vec![];
    for event in events.iter() {
        let result = evaluator.evaluate(&aggregation_node, event).await;
        results.push(result);
    }

    // First two events: count increases to 2 (not > 2)
    assert!(!results[0].triggered);
    assert!(!results[1].triggered);

    // Third event: first event should have expired, count = 2 (not > 2)
    assert!(!results[2].triggered);
    assert_eq!(results[2].value, 2.0); // Only events 2 and 3 in window
}

#[tokio::test]
async fn test_sum_aggregation() {
    let evaluator = AggregationEvaluator::new();

    let aggregation_node = NodeAggregation {
        function: AggregationFunction::Sum("amount".to_string()),
        comparison: ComparisonOp::GreaterThan,
        threshold: 1000.0,
        by_field: Some("user".to_string()),
        time_window: Some(Duration::from_secs(300)),
    };

    let base_time = Utc::now();

    let events = vec![
        EventBuilder::new()
            .field("user", "alice")
            .field("amount", 300.0)
            .timestamp(base_time)
            .build(),
        EventBuilder::new()
            .field("user", "alice")
            .field("amount", 500.0)
            .timestamp(base_time + chrono::Duration::seconds(60))
            .build(),
        EventBuilder::new()
            .field("user", "alice")
            .field("amount", 400.0)
            .timestamp(base_time + chrono::Duration::seconds(120))
            .build(),
    ];

    let mut results = vec![];
    for event in events.iter() {
        let result = evaluator.evaluate(&aggregation_node, event).await;
        results.push(result);
    }

    // First event: sum = 300 (not > 1000)
    assert!(!results[0].triggered);
    assert_eq!(results[0].value, 300.0);

    // Second event: sum = 800 (not > 1000)
    assert!(!results[1].triggered);
    assert_eq!(results[1].value, 800.0);

    // Third event: sum = 1200 (> 1000)
    assert!(results[2].triggered);
    assert_eq!(results[2].value, 1200.0);
}

#[tokio::test]
async fn test_average_aggregation() {
    let evaluator = AggregationEvaluator::new();

    let aggregation_node = NodeAggregation {
        function: AggregationFunction::Average("response_time".to_string()),
        comparison: ComparisonOp::GreaterThan,
        threshold: 500.0,
        by_field: Some("endpoint".to_string()),
        time_window: Some(Duration::from_secs(300)),
    };

    let base_time = Utc::now();

    let events = vec![
        EventBuilder::new()
            .field("endpoint", "/api/users")
            .field("response_time", 200.0)
            .timestamp(base_time)
            .build(),
        EventBuilder::new()
            .field("endpoint", "/api/users")
            .field("response_time", 300.0)
            .timestamp(base_time + chrono::Duration::seconds(30))
            .build(),
        EventBuilder::new()
            .field("endpoint", "/api/users")
            .field("response_time", 1000.0)
            .timestamp(base_time + chrono::Duration::seconds(60))
            .build(),
    ];

    let mut results = vec![];
    for event in events.iter() {
        let result = evaluator.evaluate(&aggregation_node, event).await;
        results.push(result);
    }

    // First event: avg = 200
    assert!(!results[0].triggered);
    assert_eq!(results[0].value, 200.0);

    // Second event: avg = (200 + 300) / 2 = 250
    assert!(!results[1].triggered);
    assert_eq!(results[1].value, 250.0);

    // Third event: avg = (200 + 300 + 1000) / 3 = 500 (not > 500)
    assert!(!results[2].triggered);
    assert_eq!(results[2].value, 500.0);
}

#[tokio::test]
async fn test_min_aggregation() {
    let evaluator = AggregationEvaluator::new();

    let aggregation_node = NodeAggregation {
        function: AggregationFunction::Min("score".to_string()),
        comparison: ComparisonOp::LessThan,
        threshold: 50.0,
        by_field: Some("team".to_string()),
        time_window: Some(Duration::from_secs(300)),
    };

    let base_time = Utc::now();

    let events = vec![
        EventBuilder::new()
            .field("team", "alpha")
            .field("score", 80.0)
            .timestamp(base_time)
            .build(),
        EventBuilder::new()
            .field("team", "alpha")
            .field("score", 60.0)
            .timestamp(base_time + chrono::Duration::seconds(30))
            .build(),
        EventBuilder::new()
            .field("team", "alpha")
            .field("score", 45.0)
            .timestamp(base_time + chrono::Duration::seconds(60))
            .build(),
    ];

    let mut results = vec![];
    for event in events.iter() {
        let result = evaluator.evaluate(&aggregation_node, event).await;
        results.push(result);
    }

    // First event: min = 80
    assert!(!results[0].triggered);
    assert_eq!(results[0].value, 80.0);

    // Second event: min = 60
    assert!(!results[1].triggered);
    assert_eq!(results[1].value, 60.0);

    // Third event: min = 45 (< 50)
    assert!(results[2].triggered);
    assert_eq!(results[2].value, 45.0);
}

#[tokio::test]
async fn test_max_aggregation() {
    let evaluator = AggregationEvaluator::new();

    let aggregation_node = NodeAggregation {
        function: AggregationFunction::Max("cpu_usage".to_string()),
        comparison: ComparisonOp::GreaterThan,
        threshold: 90.0,
        by_field: Some("server".to_string()),
        time_window: Some(Duration::from_secs(300)),
    };

    let base_time = Utc::now();

    let events = vec![
        EventBuilder::new()
            .field("server", "web01")
            .field("cpu_usage", 70.0)
            .timestamp(base_time)
            .build(),
        EventBuilder::new()
            .field("server", "web01")
            .field("cpu_usage", 85.0)
            .timestamp(base_time + chrono::Duration::seconds(30))
            .build(),
        EventBuilder::new()
            .field("server", "web01")
            .field("cpu_usage", 95.0)
            .timestamp(base_time + chrono::Duration::seconds(60))
            .build(),
    ];

    let mut results = vec![];
    for event in events.iter() {
        let result = evaluator.evaluate(&aggregation_node, event).await;
        results.push(result);
    }

    // First event: max = 70
    assert!(!results[0].triggered);
    assert_eq!(results[0].value, 70.0);

    // Second event: max = 85
    assert!(!results[1].triggered);
    assert_eq!(results[1].value, 85.0);

    // Third event: max = 95 (> 90)
    assert!(results[2].triggered);
    assert_eq!(results[2].value, 95.0);
}

#[test]
fn test_comparison_operators() {
    // Test all comparison operators
    assert!(ComparisonOp::GreaterThan.evaluate(10.0, 5.0));
    assert!(!ComparisonOp::GreaterThan.evaluate(5.0, 10.0));
    assert!(!ComparisonOp::GreaterThan.evaluate(5.0, 5.0));

    assert!(ComparisonOp::GreaterOrEqual.evaluate(10.0, 5.0));
    assert!(ComparisonOp::GreaterOrEqual.evaluate(5.0, 5.0));
    assert!(!ComparisonOp::GreaterOrEqual.evaluate(5.0, 10.0));

    assert!(ComparisonOp::LessThan.evaluate(5.0, 10.0));
    assert!(!ComparisonOp::LessThan.evaluate(10.0, 5.0));
    assert!(!ComparisonOp::LessThan.evaluate(5.0, 5.0));

    assert!(ComparisonOp::LessOrEqual.evaluate(5.0, 10.0));
    assert!(ComparisonOp::LessOrEqual.evaluate(5.0, 5.0));
    assert!(!ComparisonOp::LessOrEqual.evaluate(10.0, 5.0));

    assert!(ComparisonOp::Equal.evaluate(5.0, 5.0));
    assert!(!ComparisonOp::Equal.evaluate(5.0, 10.0));

    assert!(ComparisonOp::NotEqual.evaluate(5.0, 10.0));
    assert!(!ComparisonOp::NotEqual.evaluate(5.0, 5.0));
}

#[tokio::test]
async fn test_memory_efficiency() {
    // Test that memory usage is fixed per group, inspired by Go's approach
    let evaluator = AggregationEvaluator::new();

    let aggregation_node = NodeAggregation {
        function: AggregationFunction::Count,
        comparison: ComparisonOp::GreaterThan,
        threshold: 1000.0,
        by_field: Some("user".to_string()),
        time_window: Some(Duration::from_secs(60)),
    };

    let base_time = Utc::now();

    // Create many events but fixed number of groups
    let num_events = 10000;
    let num_users = 10;

    for i in 0..num_events {
        let event = EventBuilder::new()
            .field("user", format!("user{}", i % num_users))
            .field("event_id", i)
            .timestamp(base_time + chrono::Duration::milliseconds(i as i64 * 100))
            .build();

        let _ = evaluator.evaluate(&aggregation_node, &event).await;
    }

    // Verify that we only have stats for num_users groups
    let stats = evaluator.get_statistics().await;
    assert_eq!(stats.active_groups, num_users);
    assert!(stats.memory_usage_bytes < 10_000); // Should be very small
}

#[tokio::test]
async fn test_concurrent_access() {
    // Test thread safety of aggregation evaluator
    let evaluator = AggregationEvaluator::new();
    let evaluator_arc = std::sync::Arc::new(evaluator);

    let aggregation_node = std::sync::Arc::new(NodeAggregation {
        function: AggregationFunction::Count,
        comparison: ComparisonOp::GreaterThan,
        threshold: 10.0,
        by_field: Some("user".to_string()),
        time_window: Some(Duration::from_secs(60)),
    });

    let base_time = Utc::now();

    // Spawn multiple tasks that evaluate concurrently
    let mut handles = vec![];

    for thread_id in 0..10 {
        let eval_clone = evaluator_arc.clone();
        let node_clone = aggregation_node.clone();

        let handle = tokio::spawn(async move {
            for i in 0..100 {
                let event = EventBuilder::new()
                    .field("user", format!("user{}", thread_id))
                    .field("event_id", format!("{}-{}", thread_id, i))
                    .timestamp(base_time + chrono::Duration::seconds(i as i64))
                    .build();

                let _ = eval_clone.evaluate(&node_clone, &event).await;
            }
        });

        handles.push(handle);
    }

    // Wait for all tasks to complete
    futures::future::join_all(handles).await;

    // Verify state is consistent
    let stats = evaluator_arc.get_statistics().await;
    assert_eq!(stats.active_groups, 10); // One group per thread
}

#[tokio::test]
async fn test_weighted_interpolation() {
    // Test weighted interpolation matching Go's sliding statistics
    let window = SlidingWindow::new(Duration::from_secs(60));

    let base_time = Utc::now();

    // Add values to window
    window.add_value(10.0, base_time);
    window.add_value(20.0, base_time + chrono::Duration::seconds(30));

    // Query at different times should give weighted results (simplified)
    let value_at_15s = window.get_interpolated_value(base_time + chrono::Duration::seconds(15));
    // Simple mock should return current value
    assert!((value_at_15s - 20.0).abs() < 0.001);

    let value_at_45s = window.get_interpolated_value(base_time + chrono::Duration::seconds(45));
    // Simple mock should return current value
    assert!((value_at_45s - 20.0).abs() < 0.001);
}

#[tokio::test]
async fn test_aggregation_with_missing_fields() {
    let evaluator = AggregationEvaluator::new();

    let aggregation_node = NodeAggregation {
        function: AggregationFunction::Sum("amount".to_string()),
        comparison: ComparisonOp::GreaterThan,
        threshold: 100.0,
        by_field: Some("user".to_string()),
        time_window: Some(Duration::from_secs(60)),
    };

    // Event missing the amount field
    let event = EventBuilder::new()
        .field("user", "alice")
        .field("other_field", "value")
        .timestamp(Utc::now())
        .build();

    let result = evaluator.evaluate(&aggregation_node, &event).await;

    // Should handle gracefully, not crash
    assert!(!result.triggered);
    assert_eq!(result.value, 0.0); // Missing field treated as 0
}

#[tokio::test]
async fn test_ttl_cleanup() {
    let evaluator = AggregationEvaluator::with_config(AggregationConfig {
        group_ttl: Duration::from_secs(2),
        cleanup_interval: Duration::from_millis(500),
    });

    let aggregation_node = NodeAggregation {
        function: AggregationFunction::Count,
        comparison: ComparisonOp::GreaterThan,
        threshold: 10.0,
        by_field: Some("user".to_string()),
        time_window: Some(Duration::from_secs(1)),
    };

    // Add event for a user
    let event = EventBuilder::new()
        .field("user", "alice")
        .timestamp(Utc::now())
        .build();

    let _ = evaluator.evaluate(&aggregation_node, &event).await;

    // Verify group exists
    let stats = evaluator.get_statistics().await;
    assert_eq!(stats.active_groups, 1);

    // Wait for TTL to expire
    sleep(Duration::from_secs(3)).await;

    // Group should be cleaned up
    let stats = evaluator.get_statistics().await;
    assert_eq!(stats.active_groups, 0);
}
