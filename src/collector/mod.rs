//! Jolokia JMX 메트릭 수집 모듈
//!
//! Java 애플리케이션의 Jolokia 엔드포인트에서 JMX 메트릭을 수집합니다.
//!
//! # Example
//!
//! ```ignore
//! use rjmx_exporter::collector::{JolokiaClient, CollectConfig};
//!
//! let client = JolokiaClient::new("http://localhost:8778/jolokia", 5000)?;
//! let response = client.read_mbean("java.lang:type=Memory", None).await?;
//! ```

mod client;
mod parser;

pub use client::{JolokiaClient, RetryConfig};
pub use parser::{
    parse_bulk_response, parse_response, AttributeValue, CollectResult, JolokiaResponse,
    MBeanValue, ObjectName, RequestInfo,
};

/// MBean 수집 설정
#[derive(Debug, Clone)]
pub struct CollectConfig {
    /// 조회할 MBean ObjectName 목록
    pub mbeans: Vec<String>,
    /// 특정 속성만 조회 (None이면 전체)
    pub attributes: Option<Vec<String>>,
    /// 요청 타임아웃 (밀리초)
    pub timeout_ms: u64,
}

impl Default for CollectConfig {
    fn default() -> Self {
        Self {
            mbeans: vec![],
            attributes: None,
            timeout_ms: 5000,
        }
    }
}

/// Collector 구조체 - 설정 기반 수집 래퍼
pub struct Collector {
    client: JolokiaClient,
    config: CollectConfig,
}

impl Collector {
    /// 새 Collector 생성
    pub fn new(base_url: &str, config: CollectConfig) -> CollectResult<Self> {
        let client = JolokiaClient::new(base_url, config.timeout_ms)?;
        Ok(Self { client, config })
    }

    /// 설정된 MBean들 수집
    pub async fn collect(&self) -> Vec<(String, CollectResult<JolokiaResponse>)> {
        self.client.collect_with_fallback(&self.config.mbeans).await
    }

    /// Bulk 수집 (단일 HTTP 요청)
    pub async fn collect_bulk(&self) -> CollectResult<Vec<JolokiaResponse>> {
        let mbeans: Vec<(&str, Option<&[String]>)> = self
            .config
            .mbeans
            .iter()
            .map(|m| (m.as_str(), self.config.attributes.as_deref()))
            .collect();

        self.client.read_mbeans_bulk(&mbeans).await
    }

    /// 클라이언트 참조 반환
    pub fn client(&self) -> &JolokiaClient {
        &self.client
    }
}
