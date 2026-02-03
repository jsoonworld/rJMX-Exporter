//! Jolokia JMX metrics collection module
//!
//! Collects JMX metrics from Java application's Jolokia endpoint.
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

/// MBean collection configuration
#[derive(Debug, Clone)]
pub struct CollectConfig {
    /// List of MBean ObjectNames to query
    pub mbeans: Vec<String>,
    /// Specific attributes to query (None for all attributes)
    pub attributes: Option<Vec<String>>,
    /// Request timeout in milliseconds
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

/// Collector struct - configuration-based collection wrapper
pub struct Collector {
    client: JolokiaClient,
    config: CollectConfig,
}

impl Collector {
    /// Create a new Collector
    pub fn new(base_url: &str, config: CollectConfig) -> CollectResult<Self> {
        let client = JolokiaClient::new(base_url, config.timeout_ms)?;
        Ok(Self { client, config })
    }

    /// Collect configured MBeans
    pub async fn collect(&self) -> Vec<(String, CollectResult<JolokiaResponse>)> {
        self.client
            .collect_with_fallback(&self.config.mbeans, self.config.attributes.as_deref())
            .await
    }

    /// Bulk collection (single HTTP request)
    pub async fn collect_bulk(&self) -> CollectResult<Vec<JolokiaResponse>> {
        let mbeans: Vec<(&str, Option<&[String]>)> = self
            .config
            .mbeans
            .iter()
            .map(|m| (m.as_str(), self.config.attributes.as_deref()))
            .collect();

        self.client.read_mbeans_bulk(&mbeans).await
    }

    /// Return reference to client
    pub fn client(&self) -> &JolokiaClient {
        &self.client
    }
}
