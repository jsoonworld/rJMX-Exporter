//! Jolokia HTTP 클라이언트
//!
//! Connection pooling과 타임아웃을 지원하는 비동기 HTTP 클라이언트입니다.

use reqwest::{Client, ClientBuilder};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tracing::{debug, instrument, warn};

use super::parser::{parse_bulk_response, parse_response, CollectResult, JolokiaResponse};
use crate::error::CollectorError;

/// Jolokia HTTP 클라이언트
#[derive(Clone)]
pub struct JolokiaClient {
    client: Client,
    base_url: String,
    #[allow(dead_code)]
    default_timeout: Duration,
    auth: Option<(String, String)>,
}

/// Jolokia 요청 구조체
#[derive(Debug, Serialize)]
struct JolokiaRequest {
    #[serde(rename = "type")]
    request_type: String,
    mbean: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    attribute: Option<AttributeSpec>,
}

#[derive(Debug, Serialize)]
#[serde(untagged)]
enum AttributeSpec {
    Single(String),
    Multiple(Vec<String>),
}

/// 재시도 설정
#[derive(Debug, Clone)]
pub struct RetryConfig {
    /// 최대 재시도 횟수
    pub max_retries: u32,
    /// 초기 지연 시간
    pub initial_delay: Duration,
    /// 최대 지연 시간
    pub max_delay: Duration,
    /// 지연 시간 증가 배수
    pub multiplier: f64,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_retries: 3,
            initial_delay: Duration::from_millis(100),
            max_delay: Duration::from_secs(2),
            multiplier: 2.0,
        }
    }
}

impl JolokiaClient {
    /// 새 클라이언트 생성
    ///
    /// # Arguments
    /// * `base_url` - Jolokia 엔드포인트 URL (예: "http://localhost:8778/jolokia")
    /// * `timeout_ms` - 기본 타임아웃 (밀리초)
    ///
    /// # Example
    /// ```ignore
    /// let client = JolokiaClient::new("http://localhost:8778/jolokia", 5000)?;
    /// ```
    pub fn new(base_url: &str, timeout_ms: u64) -> CollectResult<Self> {
        let client = ClientBuilder::new()
            .timeout(Duration::from_millis(timeout_ms))
            .pool_max_idle_per_host(10)
            .pool_idle_timeout(Duration::from_secs(30))
            .build()
            .map_err(CollectorError::HttpClientInit)?;

        Ok(Self {
            client,
            base_url: base_url.trim_end_matches('/').to_string(),
            default_timeout: Duration::from_millis(timeout_ms),
            auth: None,
        })
    }

    /// Basic Auth 설정
    pub fn with_auth(mut self, username: &str, password: &str) -> Self {
        self.auth = Some((username.to_string(), password.to_string()));
        self
    }

    /// 단일 MBean 조회
    #[instrument(skip(self), fields(mbean = %mbean))]
    pub async fn read_mbean(
        &self,
        mbean: &str,
        attributes: Option<&[String]>,
    ) -> CollectResult<JolokiaResponse> {
        let request = JolokiaRequest {
            request_type: "read".to_string(),
            mbean: mbean.to_string(),
            attribute: attributes.map(|attrs| {
                if attrs.len() == 1 {
                    AttributeSpec::Single(attrs[0].clone())
                } else {
                    AttributeSpec::Multiple(attrs.to_vec())
                }
            }),
        };

        debug!("Sending Jolokia read request");

        let mut req = self.client.post(&self.base_url).json(&request);

        if let Some((username, password)) = &self.auth {
            req = req.basic_auth(username, Some(password));
        }

        let response = req.send().await.map_err(CollectorError::HttpRequest)?;

        let status = response.status();
        if !status.is_success() {
            return Err(CollectorError::HttpStatus(status.as_u16()));
        }

        let body = response
            .text()
            .await
            .map_err(CollectorError::HttpResponse)?;

        parse_response(&body)
    }

    /// Bulk Read - 여러 MBean 일괄 조회
    #[instrument(skip(self, mbeans), fields(count = mbeans.len()))]
    pub async fn read_mbeans_bulk(
        &self,
        mbeans: &[(&str, Option<&[String]>)],
    ) -> CollectResult<Vec<JolokiaResponse>> {
        if mbeans.is_empty() {
            return Ok(vec![]);
        }

        let requests: Vec<JolokiaRequest> = mbeans
            .iter()
            .map(|(mbean, attrs)| JolokiaRequest {
                request_type: "read".to_string(),
                mbean: mbean.to_string(),
                attribute: attrs.map(|a| {
                    if a.len() == 1 {
                        AttributeSpec::Single(a[0].clone())
                    } else {
                        AttributeSpec::Multiple(a.to_vec())
                    }
                }),
            })
            .collect();

        debug!(
            "Sending Jolokia bulk read request for {} mbeans",
            requests.len()
        );

        let mut req = self.client.post(&self.base_url).json(&requests);

        if let Some((username, password)) = &self.auth {
            req = req.basic_auth(username, Some(password));
        }

        let response = req.send().await.map_err(CollectorError::HttpRequest)?;

        let status = response.status();
        if !status.is_success() {
            return Err(CollectorError::HttpStatus(status.as_u16()));
        }

        let body = response
            .text()
            .await
            .map_err(CollectorError::HttpResponse)?;

        parse_bulk_response(&body)
    }

    /// MBean 목록 조회 (Search)
    #[instrument(skip(self))]
    pub async fn search_mbeans(&self, pattern: &str) -> CollectResult<Vec<String>> {
        #[derive(Serialize)]
        struct SearchRequest {
            #[serde(rename = "type")]
            request_type: String,
            mbean: String,
        }

        let request = SearchRequest {
            request_type: "search".to_string(),
            mbean: pattern.to_string(),
        };

        let mut req = self.client.post(&self.base_url).json(&request);

        if let Some((username, password)) = &self.auth {
            req = req.basic_auth(username, Some(password));
        }

        let response = req.send().await.map_err(CollectorError::HttpRequest)?;

        let body = response
            .text()
            .await
            .map_err(CollectorError::HttpResponse)?;

        #[derive(Deserialize)]
        struct SearchResponse {
            value: Vec<String>,
            status: u16,
        }

        let parsed: SearchResponse =
            serde_json::from_str(&body).map_err(|e| CollectorError::JsonParse(e.to_string()))?;

        if parsed.status != 200 {
            return Err(CollectorError::JolokiaError {
                status: parsed.status,
                message: "Search failed".to_string(),
            });
        }

        Ok(parsed.value)
    }

    /// 재시도 로직이 포함된 단일 MBean 조회
    pub async fn read_mbean_with_retry(
        &self,
        mbean: &str,
        attributes: Option<&[String]>,
        config: &RetryConfig,
    ) -> CollectResult<JolokiaResponse> {
        let mut delay = config.initial_delay;
        let mut last_error = None;

        for attempt in 0..=config.max_retries {
            match self.read_mbean(mbean, attributes).await {
                Ok(result) => return Ok(result),
                Err(e) => {
                    if !e.is_retryable() {
                        return Err(e);
                    }

                    last_error = Some(e);

                    if attempt < config.max_retries {
                        warn!(
                            attempt = attempt + 1,
                            max = config.max_retries,
                            delay_ms = delay.as_millis() as u64,
                            "Request failed, retrying"
                        );
                        tokio::time::sleep(delay).await;
                        delay = std::cmp::min(
                            Duration::from_secs_f64(delay.as_secs_f64() * config.multiplier),
                            config.max_delay,
                        );
                    }
                }
            }
        }

        Err(last_error.unwrap_or(CollectorError::MaxRetriesExceeded))
    }

    /// Fallback이 있는 수집 - 부분 실패 허용
    pub async fn collect_with_fallback(
        &self,
        mbeans: &[String],
    ) -> Vec<(String, CollectResult<JolokiaResponse>)> {
        let mut results = Vec::new();

        for mbean in mbeans {
            let result = self.read_mbean(mbean, None).await;

            match &result {
                Ok(response) if response.status == 200 => {
                    debug!(mbean = %mbean, "MBean collected successfully");
                }
                Ok(response) => {
                    warn!(
                        mbean = %mbean,
                        status = response.status,
                        error = ?response.error,
                        "MBean collection returned non-200 status"
                    );
                }
                Err(e) => {
                    tracing::error!(
                        mbean = %mbean,
                        error = %e,
                        "Failed to collect MBean"
                    );
                }
            }

            results.push((mbean.clone(), result));
        }

        results
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_new() {
        let client = JolokiaClient::new("http://localhost:8778/jolokia", 5000);
        assert!(client.is_ok());
    }

    #[test]
    fn test_client_with_auth() {
        let client = JolokiaClient::new("http://localhost:8778/jolokia", 5000)
            .unwrap()
            .with_auth("user", "pass");
        assert!(client.auth.is_some());
    }

    #[test]
    fn test_retry_config_default() {
        let config = RetryConfig::default();
        assert_eq!(config.max_retries, 3);
        assert_eq!(config.initial_delay, Duration::from_millis(100));
    }
}
