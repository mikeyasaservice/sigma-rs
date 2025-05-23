#[cfg(feature = "service")]
mod service_tests {
    use sigma_rs::{SigmaEngine, SigmaEngineBuilder};
    use sigma_rs::service::SigmaService;
    use axum::body::Body;
    use axum::http::{Request, Method, StatusCode};
    use tower::ServiceExt;
    use tempfile::TempDir;
    use std::sync::Arc;

    async fn create_test_engine() -> Arc<SigmaEngine> {
        let temp_dir = TempDir::new().unwrap();
        let rules_dir = temp_dir.path().join("rules");
        std::fs::create_dir(&rules_dir).unwrap();
        
        // Create a simple test rule
        let rule_content = r#"
title: Test Rule
status: experimental
logsource:
    product: test
detection:
    keywords:
        - "test"
    condition: keywords
"#;
        std::fs::write(rules_dir.join("test.yml"), rule_content).unwrap();
        
        let builder = SigmaEngineBuilder::new()
            .add_rule_dir(rules_dir.to_string_lossy());
        let engine = builder.build().await.unwrap();
        Arc::new(engine)
    }

    #[tokio::test]
    async fn test_health_endpoint() {
        let engine = create_test_engine().await;
        let service = SigmaService::new(engine);
        let app = service.router();
        
        let response = app
            .oneshot(
                Request::builder()
                    .method(Method::GET)
                    .uri("/health")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_evaluate_endpoint() {
        let engine = create_test_engine().await;
        let service = SigmaService::new(engine);
        let app = service.router();
        
        let event_data = serde_json::json!({
            "event": {
                "message": "test event"
            }
        });
        
        let response = app
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/evaluate")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_string(&event_data).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();
        
        assert_eq!(response.status(), StatusCode::OK);
    }
}