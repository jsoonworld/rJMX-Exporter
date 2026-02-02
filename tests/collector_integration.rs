//! Collector 통합 테스트
//!
//! wiremock을 사용한 HTTP 모킹 테스트

use rjmx_exporter::collector::{JolokiaClient, MBeanValue};
use serde_json::json;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn test_read_mbean_success() {
    let mock_server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/jolokia"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "request": {
                "mbean": "java.lang:type=Memory",
                "type": "read"
            },
            "value": {
                "HeapMemoryUsage": {
                    "used": 52428800_i64,
                    "max": 4294967296_i64
                }
            },
            "timestamp": 1609459200,
            "status": 200
        })))
        .mount(&mock_server)
        .await;

    let url = format!("{}/jolokia", mock_server.uri());
    let client = JolokiaClient::new(&url, 5000).unwrap();
    let response = client
        .read_mbean("java.lang:type=Memory", None)
        .await
        .unwrap();

    assert_eq!(response.status, 200);
    assert!(matches!(response.value, MBeanValue::Composite(_)));
}

#[tokio::test]
async fn test_bulk_read() {
    let mock_server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/jolokia"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([
            {
                "request": {"mbean": "java.lang:type=Threading", "type": "read"},
                "value": 42,
                "status": 200,
                "timestamp": 1609459200
            },
            {
                "request": {"mbean": "java.lang:type=Memory", "type": "read"},
                "value": {"used": 1000000},
                "status": 200,
                "timestamp": 1609459200
            }
        ])))
        .mount(&mock_server)
        .await;

    let url = format!("{}/jolokia", mock_server.uri());
    let client = JolokiaClient::new(&url, 5000).unwrap();
    let responses = client
        .read_mbeans_bulk(&[
            ("java.lang:type=Threading", None),
            ("java.lang:type=Memory", None),
        ])
        .await
        .unwrap();

    assert_eq!(responses.len(), 2);
    assert_eq!(responses[0].status, 200);
    assert_eq!(responses[1].status, 200);
}

#[tokio::test]
async fn test_timeout_handling() {
    let mock_server = MockServer::start().await;

    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200).set_delay(std::time::Duration::from_secs(10)))
        .mount(&mock_server)
        .await;

    let url = format!("{}/jolokia", mock_server.uri());
    let client = JolokiaClient::new(&url, 100).unwrap();
    let result = client.read_mbean("java.lang:type=Memory", None).await;

    assert!(result.is_err());
}

#[tokio::test]
async fn test_error_response_handling() {
    let mock_server = MockServer::start().await;

    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "request": {"mbean": "invalid:type=NotFound", "type": "read"},
            "error_type": "javax.management.InstanceNotFoundException",
            "error": "No MBean found",
            "status": 404
        })))
        .mount(&mock_server)
        .await;

    let url = format!("{}/jolokia", mock_server.uri());
    let client = JolokiaClient::new(&url, 5000).unwrap();
    let response = client
        .read_mbean("invalid:type=NotFound", None)
        .await
        .unwrap();

    assert_eq!(response.status, 404);
    assert!(response.error.is_some());
}

#[tokio::test]
async fn test_connection_pool_reuse() {
    let mock_server = MockServer::start().await;

    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "request": {"mbean": "test:type=Test", "type": "read"},
            "value": 1,
            "status": 200,
            "timestamp": 1609459200
        })))
        .expect(10)
        .mount(&mock_server)
        .await;

    let url = format!("{}/jolokia", mock_server.uri());
    let client = JolokiaClient::new(&url, 5000).unwrap();

    for _ in 0..10 {
        let result = client.read_mbean("test:type=Test", None).await;
        assert!(result.is_ok());
    }
}

#[tokio::test]
async fn test_search_mbeans() {
    let mock_server = MockServer::start().await;

    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "value": [
                "java.lang:type=GarbageCollector,name=G1 Young Generation",
                "java.lang:type=GarbageCollector,name=G1 Old Generation"
            ],
            "status": 200
        })))
        .mount(&mock_server)
        .await;

    let url = format!("{}/jolokia", mock_server.uri());
    let client = JolokiaClient::new(&url, 5000).unwrap();
    let mbeans = client
        .search_mbeans("java.lang:type=GarbageCollector,*")
        .await
        .unwrap();

    assert_eq!(mbeans.len(), 2);
}

#[tokio::test]
async fn test_http_500_error() {
    let mock_server = MockServer::start().await;

    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&mock_server)
        .await;

    let url = format!("{}/jolokia", mock_server.uri());
    let client = JolokiaClient::new(&url, 5000).unwrap();
    let result = client.read_mbean("java.lang:type=Memory", None).await;

    assert!(result.is_err());
}
