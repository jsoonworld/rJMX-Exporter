//! Scrape integration tests
//!
//! End-to-end tests for the scrape pipeline that verify:
//! - Basic scrape functionality
//! - Rule transformation
//! - Error handling

use rjmx_exporter::collector::{AttributeValue, JolokiaClient, JolokiaResponse, MBeanValue};
use rjmx_exporter::transformer::engine::{PrometheusMetric, TransformEngine};
use rjmx_exporter::transformer::formatter::PrometheusFormatter;
use rjmx_exporter::transformer::rules::{MetricType, Rule, RuleSet};
use serde_json::json;
use std::collections::HashMap;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

/// Create a mock Jolokia server that returns memory metrics
async fn create_mock_jolokia_server() -> MockServer {
    let mock_server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/jolokia"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "request": {
                "mbean": "java.lang:type=Memory",
                "attribute": "HeapMemoryUsage",
                "type": "read"
            },
            "value": {
                "init": 268435456_i64,
                "committed": 536870912_i64,
                "max": 4294967296_i64,
                "used": 123456789_i64
            },
            "timestamp": 1609459200,
            "status": 200
        })))
        .mount(&mock_server)
        .await;

    mock_server
}

/// Create a transform engine with JVM memory rules
fn create_test_transform_engine() -> TransformEngine {
    let mut ruleset = RuleSet::new();
    ruleset.add(
        Rule::builder(r"java\.lang<type=Memory><HeapMemoryUsage><(\w+)>")
            .name("jvm_memory_heap_$1_bytes")
            .metric_type(MetricType::Gauge)
            .help("JVM heap memory usage in bytes")
            .label("area", "heap")
            .build(),
    );
    ruleset.add(
        Rule::builder(r"java\.lang<type=Threading><(\w+)>")
            .name("jvm_threads_$1")
            .metric_type(MetricType::Gauge)
            .help("JVM thread metrics")
            .build(),
    );
    TransformEngine::new(ruleset)
}

/// Test basic scrape functionality
///
/// This test verifies the complete pipeline:
/// 1. Fetch data from mock Jolokia server
/// 2. Parse the response
/// 3. Transform using rules
/// 4. Format as Prometheus metrics
#[tokio::test]
async fn test_basic_scrape() {
    // 1. Start mock Jolokia server
    let mock_server = create_mock_jolokia_server().await;

    // 2. Create client and fetch data
    let url = format!("{}/jolokia", mock_server.uri());
    let client = JolokiaClient::new(&url, 5000).expect("Failed to create client");

    let response = client
        .read_mbean(
            "java.lang:type=Memory",
            Some(&["HeapMemoryUsage".to_string()]),
        )
        .await
        .expect("Failed to read MBean");

    // 3. Verify response is valid
    assert_eq!(response.status, 200);
    assert!(matches!(response.value, MBeanValue::Composite(_)));

    // 4. Transform and format
    let engine = create_test_transform_engine();
    let metrics = engine.transform(&[response]).expect("Transform failed");

    // Should produce 4 metrics: init, committed, max, used
    assert_eq!(
        metrics.len(),
        4,
        "Expected 4 metrics for composite HeapMemoryUsage"
    );

    // 5. Format as Prometheus output
    let formatter = PrometheusFormatter::new();
    let output = formatter.format(&metrics);

    // Verify output contains expected metrics
    assert!(
        output.contains("jvm_memory_heap_used_bytes"),
        "Output should contain jvm_memory_heap_used_bytes"
    );
    assert!(
        output.contains("jvm_memory_heap_max_bytes"),
        "Output should contain jvm_memory_heap_max_bytes"
    );
    assert!(
        output.contains("area=\"heap\""),
        "Output should contain heap label"
    );
    assert!(
        output.contains("# TYPE jvm_memory_heap_"),
        "Output should contain TYPE annotation"
    );
}

/// Test rule transformation with capture groups
#[tokio::test]
async fn test_rule_transformation() {
    // Create a mock response directly for testing transformation
    let mut composite_value = HashMap::new();
    composite_value.insert("used".to_string(), AttributeValue::Integer(52428800));
    composite_value.insert("max".to_string(), AttributeValue::Integer(536870912));
    composite_value.insert("committed".to_string(), AttributeValue::Integer(268435456));
    composite_value.insert("init".to_string(), AttributeValue::Integer(134217728));

    let response = JolokiaResponse {
        request: rjmx_exporter::collector::RequestInfo {
            mbean: "java.lang:type=Memory".to_string(),
            attribute: Some(serde_json::json!("HeapMemoryUsage")),
            request_type: "read".to_string(),
        },
        value: MBeanValue::Composite(composite_value),
        status: 200,
        timestamp: 1609459200,
        error: None,
        error_type: None,
    };

    let engine = create_test_transform_engine();
    let metrics = engine.transform(&[response]).expect("Transform failed");

    // Verify metrics are created with correct names
    let metric_names: Vec<&str> = metrics.iter().map(|m| m.name.as_str()).collect();

    assert!(
        metric_names.contains(&"jvm_memory_heap_used_bytes"),
        "Should contain used metric, got: {:?}",
        metric_names
    );
    assert!(
        metric_names.contains(&"jvm_memory_heap_max_bytes"),
        "Should contain max metric"
    );
    assert!(
        metric_names.contains(&"jvm_memory_heap_committed_bytes"),
        "Should contain committed metric"
    );
    assert!(
        metric_names.contains(&"jvm_memory_heap_init_bytes"),
        "Should contain init metric"
    );

    // Verify labels are applied
    for metric in &metrics {
        assert_eq!(
            metric.labels.get("area"),
            Some(&"heap".to_string()),
            "Each metric should have area=heap label"
        );
        assert_eq!(
            metric.metric_type,
            MetricType::Gauge,
            "Each metric should be a gauge"
        );
    }

    // Verify values are correct
    let used_metric = metrics
        .iter()
        .find(|m| m.name == "jvm_memory_heap_used_bytes");
    assert!(used_metric.is_some(), "Should have used_bytes metric");
    assert!(
        (used_metric.unwrap().value - 52428800.0).abs() < f64::EPSILON,
        "used_bytes should be 52428800"
    );
}

/// Test error handling when target is down
#[tokio::test]
async fn test_error_handling_target_down() {
    let mock_server = MockServer::start().await;

    // Configure mock to return 500 error
    Mock::given(method("POST"))
        .and(path("/jolokia"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&mock_server)
        .await;

    let url = format!("{}/jolokia", mock_server.uri());
    let client = JolokiaClient::new(&url, 5000).expect("Failed to create client");

    // Attempt to read from "down" target
    let result = client.read_mbean("java.lang:type=Memory", None).await;

    // Should return an error
    assert!(result.is_err(), "Should fail when target returns 500");
}

/// Test handling of Jolokia error responses (e.g., MBean not found)
#[tokio::test]
async fn test_error_handling_mbean_not_found() {
    let mock_server = MockServer::start().await;

    // Configure mock to return Jolokia error response
    Mock::given(method("POST"))
        .and(path("/jolokia"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "request": {
                "mbean": "invalid:type=NotFound",
                "type": "read"
            },
            "error_type": "javax.management.InstanceNotFoundException",
            "error": "No MBean found for invalid:type=NotFound",
            "status": 404
        })))
        .mount(&mock_server)
        .await;

    let url = format!("{}/jolokia", mock_server.uri());
    let client = JolokiaClient::new(&url, 5000).expect("Failed to create client");

    let response = client
        .read_mbean("invalid:type=NotFound", None)
        .await
        .expect("Should return response even for Jolokia errors");

    // Jolokia returns 200 HTTP status but with error in body
    assert_eq!(response.status, 404);
    assert!(response.error.is_some());
    assert!(response.error.unwrap().contains("No MBean found"));
}

/// Test transformation with multiple MBeans
#[tokio::test]
async fn test_multi_mbean_transformation() {
    // Create multiple mock responses
    let responses = vec![
        JolokiaResponse {
            request: rjmx_exporter::collector::RequestInfo {
                mbean: "java.lang:type=Memory".to_string(),
                attribute: Some(serde_json::json!("HeapMemoryUsage")),
                request_type: "read".to_string(),
            },
            value: MBeanValue::Composite({
                let mut map = HashMap::new();
                map.insert("used".to_string(), AttributeValue::Integer(100000000));
                map.insert("max".to_string(), AttributeValue::Integer(500000000));
                map
            }),
            status: 200,
            timestamp: 1609459200,
            error: None,
            error_type: None,
        },
        JolokiaResponse {
            request: rjmx_exporter::collector::RequestInfo {
                mbean: "java.lang:type=Threading".to_string(),
                attribute: Some(serde_json::json!("ThreadCount")),
                request_type: "read".to_string(),
            },
            value: MBeanValue::Number(42.0),
            status: 200,
            timestamp: 1609459200,
            error: None,
            error_type: None,
        },
    ];

    let engine = create_test_transform_engine();
    let metrics = engine.transform(&responses).expect("Transform failed");

    // Should have metrics from both MBeans
    let has_memory_metric = metrics.iter().any(|m| m.name.contains("jvm_memory_heap"));
    let has_thread_metric = metrics.iter().any(|m| m.name.contains("jvm_threads"));

    assert!(has_memory_metric, "Should have memory metrics");
    assert!(has_thread_metric, "Should have thread metrics");
}

/// Test timeout handling
#[tokio::test]
async fn test_timeout_handling() {
    let mock_server = MockServer::start().await;

    // Configure mock to delay response longer than timeout
    Mock::given(method("POST"))
        .and(path("/jolokia"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(json!({
                    "request": {"mbean": "java.lang:type=Memory", "type": "read"},
                    "value": 42,
                    "status": 200,
                    "timestamp": 1609459200
                }))
                .set_delay(std::time::Duration::from_secs(5)),
        )
        .mount(&mock_server)
        .await;

    // Create client with 100ms timeout
    let url = format!("{}/jolokia", mock_server.uri());
    let client = JolokiaClient::new(&url, 100).expect("Failed to create client");

    let result = client.read_mbean("java.lang:type=Memory", None).await;

    // Should timeout
    assert!(result.is_err(), "Should fail due to timeout");
}

/// Test that the complete scrape output matches Prometheus format
#[tokio::test]
async fn test_prometheus_format_compliance() {
    let metrics = vec![
        PrometheusMetric::new("jvm_memory_heap_used_bytes", 123456789.0)
            .with_type(MetricType::Gauge)
            .with_help("JVM heap memory used")
            .with_label("area", "heap"),
        PrometheusMetric::new("jvm_memory_heap_max_bytes", 536870912.0)
            .with_type(MetricType::Gauge)
            .with_help("JVM heap memory max")
            .with_label("area", "heap"),
        PrometheusMetric::new("jvm_threads_current", 42.0)
            .with_type(MetricType::Gauge)
            .with_help("Current thread count"),
    ];

    let formatter = PrometheusFormatter::new();
    let output = formatter.format(&metrics);

    // Verify Prometheus format compliance
    // 1. HELP lines
    assert!(output.contains("# HELP jvm_memory_heap_used_bytes JVM heap memory used"));
    assert!(output.contains("# HELP jvm_threads_current Current thread count"));

    // 2. TYPE lines
    assert!(output.contains("# TYPE jvm_memory_heap_used_bytes gauge"));
    assert!(output.contains("# TYPE jvm_threads_current gauge"));

    // 3. Metric lines with labels
    assert!(output.contains("jvm_memory_heap_used_bytes{area=\"heap\"} 123456789"));
    assert!(output.contains("jvm_memory_heap_max_bytes{area=\"heap\"} 536870912"));

    // 4. Metric line without labels
    assert!(output.contains("jvm_threads_current 42"));

    // 5. No trailing whitespace issues
    for line in output.lines() {
        assert!(
            !line.ends_with(' '),
            "Line should not end with space: {}",
            line
        );
    }
}

/// Test handling of error responses - error metrics should still be parseable
#[tokio::test]
async fn test_transform_skips_error_responses() {
    let responses = vec![
        // Error response - should be skipped
        JolokiaResponse {
            request: rjmx_exporter::collector::RequestInfo {
                mbean: "invalid:type=NotFound".to_string(),
                attribute: None,
                request_type: "read".to_string(),
            },
            value: MBeanValue::Null,
            status: 404,
            timestamp: 1609459200,
            error: Some("Not found".to_string()),
            error_type: Some("javax.management.InstanceNotFoundException".to_string()),
        },
        // Valid response - should be processed
        JolokiaResponse {
            request: rjmx_exporter::collector::RequestInfo {
                mbean: "java.lang:type=Threading".to_string(),
                attribute: Some(serde_json::json!("ThreadCount")),
                request_type: "read".to_string(),
            },
            value: MBeanValue::Number(42.0),
            status: 200,
            timestamp: 1609459200,
            error: None,
            error_type: None,
        },
    ];

    let engine = create_test_transform_engine();
    let metrics = engine
        .transform(&responses)
        .expect("Transform should succeed");

    // Only the valid response should produce metrics
    assert!(
        !metrics.is_empty(),
        "Should have metrics from valid response"
    );
    assert!(
        metrics.iter().all(|m| m.name.contains("jvm_threads")),
        "All metrics should be from the valid Threading response"
    );
}

/// Test lowercase option for metric names
#[tokio::test]
async fn test_lowercase_option() {
    let mut ruleset = RuleSet::new();
    ruleset.add(
        Rule::builder(r"java\.lang<type=Memory><HeapMemoryUsage><(\w+)>")
            .name("JVM_Memory_Heap_$1_BYTES")
            .metric_type(MetricType::Gauge)
            .build(),
    );

    let engine = TransformEngine::new(ruleset).with_lowercase_names(true);

    let response = JolokiaResponse {
        request: rjmx_exporter::collector::RequestInfo {
            mbean: "java.lang:type=Memory".to_string(),
            attribute: Some(serde_json::json!("HeapMemoryUsage")),
            request_type: "read".to_string(),
        },
        value: MBeanValue::Composite({
            let mut map = HashMap::new();
            map.insert("used".to_string(), AttributeValue::Integer(100));
            map
        }),
        status: 200,
        timestamp: 1609459200,
        error: None,
        error_type: None,
    };

    let metrics = engine.transform(&[response]).expect("Transform failed");

    assert!(!metrics.is_empty());
    // Should be lowercased
    assert!(
        metrics[0]
            .name
            .chars()
            .all(|c| c.is_lowercase() || c == '_'),
        "Metric name should be lowercase: {}",
        metrics[0].name
    );
}
