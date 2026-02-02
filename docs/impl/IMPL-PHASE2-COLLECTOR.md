# Phase 2: Data Collection 구현 계획서

> rJMX-Exporter - Jolokia JSON 파싱 및 MBean 데이터 구조 정의

---

## 1. 개요 (Overview)

Phase 2는 rJMX-Exporter의 핵심 데이터 수집 계층을 구현합니다. Jolokia HTTP 엔드포인트에서 JMX 메트릭을 수집하고, JSON 응답을 파싱하여 내부 데이터 구조로 변환하는 기능을 담당합니다.

### 범위

```text
┌─────────────────────────────────────────────────────────────────────┐
│                         Phase 2 Scope                               │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│   Java App (Jolokia)  ──HTTP/JSON──►  Collector Module              │
│                                       ├── client.rs (HTTP request)  │
│                                       ├── parser.rs (JSON parsing)  │
│                                       └── mod.rs (unified interface)│
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

### Phase 1과의 관계

- **Phase 1 (Foundation)**: Tokio + Axum 기본 서버, 프로젝트 구조
- **Phase 2 (Data Collection)**: Jolokia 데이터 수집 ← **현재 단계**
- **Phase 3 (Transform Engine)**: MBean → Prometheus 변환

---

## 2. 목표 (Goals)

### 성능 목표

| 메트릭 | 목표값 | 측정 방법 |
| --- | --- | --- |
| Scrape Latency | < 10ms | Jolokia 요청 → 파싱 완료 시간 |
| Memory per Request | < 1MB | 단일 수집 요청당 메모리 사용량 |
| Connection Reuse | 100% | Connection Pool 활용률 |

### 기능 목표

| 항목 | 설명 | 우선순위 |
|------|------|----------|
| 단일 MBean 조회 | `/jolokia/read/{mbean}` 지원 | P0 |
| Bulk Read | 여러 MBean 일괄 조회 | P0 |
| Attribute 필터링 | 특정 속성만 조회 | P1 |
| 에러 복구 | 부분 실패 시 graceful degradation | P1 |

### 비기능 목표

- 모든 public API에 문서화
- 단위 테스트 커버리지 80% 이상
- `unwrap()` 및 `panic!()` 사용 금지

---

## 3. Jolokia API 분석 (Jolokia API Analysis)

### 3.1 Jolokia 프로토콜 개요

Jolokia는 JMX를 HTTP/JSON으로 노출하는 에이전트입니다. 주요 작업(Operation)은 다음과 같습니다:

| Operation | 설명 | 용도 |
|-----------|------|------|
| `read` | MBean 속성 읽기 | 메트릭 수집 |
| `list` | MBean 목록 조회 | 디스커버리 |
| `search` | MBean 검색 | 패턴 매칭 |
| `exec` | MBean 메서드 실행 | (사용 안 함) |

### 3.2 Read 요청 형식

#### 단일 MBean 조회

```http
POST /jolokia/ HTTP/1.1
Content-Type: application/json

{
  "type": "read",
  "mbean": "java.lang:type=Memory",
  "attribute": "HeapMemoryUsage"
}
```

#### 응답

```json
{
  "request": {
    "mbean": "java.lang:type=Memory",
    "attribute": "HeapMemoryUsage",
    "type": "read"
  },
  "value": {
    "init": 268435456,
    "committed": 268435456,
    "max": 4294967296,
    "used": 52428800
  },
  "timestamp": 1609459200,
  "status": 200
}
```

### 3.3 Bulk Read 요청

여러 MBean을 한 번의 HTTP 요청으로 조회하여 네트워크 오버헤드를 줄입니다.

#### 요청

```json
[
  {
    "type": "read",
    "mbean": "java.lang:type=Memory",
    "attribute": "HeapMemoryUsage"
  },
  {
    "type": "read",
    "mbean": "java.lang:type=Threading",
    "attribute": "ThreadCount"
  },
  {
    "type": "read",
    "mbean": "java.lang:type=OperatingSystem",
    "attribute": ["ProcessCpuLoad", "SystemCpuLoad"]
  }
]
```

#### 응답

```json
[
  {
    "request": { "mbean": "java.lang:type=Memory", ... },
    "value": { "init": 268435456, ... },
    "status": 200
  },
  {
    "request": { "mbean": "java.lang:type=Threading", ... },
    "value": 42,
    "status": 200
  },
  {
    "request": { "mbean": "java.lang:type=OperatingSystem", ... },
    "value": { "ProcessCpuLoad": 0.15, "SystemCpuLoad": 0.25 },
    "status": 200
  }
]
```

### 3.4 에러 응답

```json
{
  "request": { "mbean": "invalid:type=NotFound", ... },
  "error_type": "javax.management.InstanceNotFoundException",
  "error": "No MBean found for 'invalid:type=NotFound'",
  "status": 404
}
```

### 3.5 Path 파라미터 (CompositeData 접근)

Jolokia는 CompositeData의 특정 키에 직접 접근할 수 있는 path 파라미터를 지원합니다.

#### 요청
```json
{
  "type": "read",
  "mbean": "java.lang:type=Memory",
  "attribute": "HeapMemoryUsage",
  "path": "used"
}
```

#### 응답
```json
{
  "request": {
    "mbean": "java.lang:type=Memory",
    "attribute": "HeapMemoryUsage",
    "path": "used",
    "type": "read"
  },
  "value": 52428800,
  "timestamp": 1609459200,
  "status": 200
}
```

> **Phase 2 지원 범위**: Path 파라미터는 P1 우선순위로, 기본 기능 완료 후 추가 구현 예정.

### 3.6 와일드카드 패턴

```json
{
  "type": "read",
  "mbean": "java.lang:type=GarbageCollector,name=*"
}
```

응답 시 `value`가 객체의 맵으로 반환됩니다:

```json
{
  "value": {
    "java.lang:type=GarbageCollector,name=G1 Young Generation": {
      "CollectionCount": 42,
      "CollectionTime": 1234
    },
    "java.lang:type=GarbageCollector,name=G1 Old Generation": {
      "CollectionCount": 5,
      "CollectionTime": 567
    }
  },
  "status": 200
}
```

---

## 3.7 Sub-Phase 구분

Phase 2는 다음과 같이 세분화하여 진행합니다:

| Sub-Phase | 범위 | 완료 조건 |
|-----------|------|-----------|
| **2A: 요청 빌더/클라이언트** | `JolokiaClient::new()`, HTTP 요청 전송 | 단일 MBean 요청 성공 |
| **2B: 파서/데이터 모델** | `JolokiaResponse`, `MBeanValue`, JSON 파싱 | 모든 값 타입 파싱 테스트 통과 |
| **2C: 에러/리트라이/통합** | `CollectorError`, 재시도 로직, wiremock 테스트 | 에러 시나리오 테스트 통과 |

---

## 4. 구현 항목 (Implementation Items)

### 4.1 파일 구조

```
src/collector/
├── mod.rs          # 모듈 공개 인터페이스, Collector trait
├── client.rs       # JolokiaClient: HTTP 요청 담당
└── parser.rs       # 응답 파싱, 데이터 구조 정의
```

### 4.2 collector/mod.rs

모듈의 공개 인터페이스를 정의합니다.

```rust
//! Jolokia JMX 메트릭 수집 모듈
//!
//! Java 애플리케이션의 Jolokia 엔드포인트에서 JMX 메트릭을 수집합니다.

mod client;
mod parser;

pub use client::JolokiaClient;
pub use parser::{
    JolokiaResponse, MBeanValue, AttributeValue,
    parse_response, parse_bulk_response,
};

use crate::error::CollectorError;

/// 메트릭 수집 결과
pub type CollectResult<T> = Result<T, CollectorError>;

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
```

### 4.3 collector/client.rs

Reqwest 기반 HTTP 클라이언트를 구현합니다.

```rust
//! Jolokia HTTP 클라이언트
//!
//! Connection pooling과 타임아웃을 지원하는 비동기 HTTP 클라이언트입니다.

use reqwest::{Client, ClientBuilder};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tracing::{debug, instrument, warn};

use super::{CollectConfig, CollectResult};
use super::parser::{JolokiaResponse, parse_response, parse_bulk_response};
use crate::error::CollectorError;

/// Jolokia HTTP 클라이언트
#[derive(Clone)]
pub struct JolokiaClient {
    client: Client,
    base_url: String,
    default_timeout: Duration,
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

impl JolokiaClient {
    /// 새 클라이언트 생성
    ///
    /// # Arguments
    /// * `base_url` - Jolokia 엔드포인트 URL
    ///   - **MUST** include `/jolokia` path (예: "http://localhost:8778/jolokia")
    ///   - Trailing slash is automatically removed
    /// * `timeout_ms` - 기본 타임아웃 (밀리초)
    ///
    /// # Example
    /// ```
    /// // Correct: URL includes /jolokia path
    /// let client = JolokiaClient::new("http://localhost:8778/jolokia", 5000)?;
    ///
    /// // Wrong: Missing /jolokia path - will fail on requests
    /// // let client = JolokiaClient::new("http://localhost:8778", 5000)?;
    /// ```
    ///
    /// # Note
    /// HTTP/2 prior knowledge is NOT enabled by default because most Jolokia
    /// deployments don't support h2c (unencrypted HTTP/2). Use `with_http2()`
    /// to enable it if your server supports it.
    pub fn new(base_url: &str, timeout_ms: u64) -> CollectResult<Self> {
        let client = ClientBuilder::new()
            .timeout(Duration::from_millis(timeout_ms))
            .pool_max_idle_per_host(10)
            .pool_idle_timeout(Duration::from_secs(30))
            // Note: http2_prior_knowledge() is NOT enabled by default
            // Enable it manually if your Jolokia server supports h2c
            .build()
            .map_err(CollectorError::HttpClientInit)?;

        Ok(Self {
            client,
            base_url: base_url.trim_end_matches('/').to_string(),
            default_timeout: Duration::from_millis(timeout_ms),
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

        let response = self
            .client
            .post(&self.base_url)
            .json(&request)
            .send()
            .await
            .map_err(CollectorError::HttpRequest)?;

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

        debug!("Sending Jolokia bulk read request for {} mbeans", requests.len());

        let response = self
            .client
            .post(&self.base_url)
            .json(&requests)
            .send()
            .await
            .map_err(CollectorError::HttpRequest)?;

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

        let response = self
            .client
            .post(&self.base_url)
            .json(&request)
            .send()
            .await
            .map_err(CollectorError::HttpRequest)?;

        let body = response
            .text()
            .await
            .map_err(CollectorError::HttpResponse)?;

        // 검색 응답 파싱
        #[derive(Deserialize)]
        struct SearchResponse {
            value: Vec<String>,
            status: u16,
        }

        let parsed: SearchResponse = serde_json::from_str(&body)
            .map_err(|e| CollectorError::JsonParse(e.to_string()))?;

        if parsed.status != 200 {
            return Err(CollectorError::JolokiaError {
                status: parsed.status,
                message: "Search failed".to_string(),
            });
        }

        Ok(parsed.value)
    }
}
```

### 4.4 collector/parser.rs

JSON 파싱 및 데이터 구조를 정의합니다.

```rust
//! Jolokia JSON 응답 파서
//!
//! Jolokia API 응답을 파싱하여 내부 데이터 구조로 변환합니다.

use serde::{Deserialize, Deserializer};
use serde_json::Value;
use std::collections::HashMap;
use tracing::warn;

use super::CollectResult;
use crate::error::CollectorError;

/// Jolokia API 응답 구조체
#[derive(Debug, Clone)]
pub struct JolokiaResponse {
    /// 요청 정보
    pub request: RequestInfo,
    /// 응답 값
    pub value: MBeanValue,
    /// 응답 상태 코드
    pub status: u16,
    /// 타임스탬프 (Unix epoch)
    pub timestamp: u64,
    /// 에러 메시지 (실패 시)
    pub error: Option<String>,
    /// 에러 타입 (실패 시)
    pub error_type: Option<String>,
}

/// 요청 정보
#[derive(Debug, Clone, Deserialize)]
pub struct RequestInfo {
    /// MBean ObjectName
    pub mbean: String,
    /// 조회한 속성 (단일 또는 복수)
    #[serde(default)]
    pub attribute: Option<Value>,
    /// 요청 타입
    #[serde(rename = "type")]
    pub request_type: String,
}

/// MBean 값 - 다양한 형태를 지원
#[derive(Debug, Clone)]
pub enum MBeanValue {
    /// 단순 숫자 값
    Number(f64),
    /// 문자열 값
    String(String),
    /// 불리언 값
    Boolean(bool),
    /// Null 값
    Null,
    /// 복합 객체 (CompositeData)
    Composite(HashMap<String, AttributeValue>),
    /// 배열
    Array(Vec<AttributeValue>),
    /// 와일드카드 결과 (MBean ObjectName -> 속성 맵)
    Wildcard(HashMap<String, HashMap<String, AttributeValue>>),
}

/// 개별 속성 값
#[derive(Debug, Clone)]
pub enum AttributeValue {
    /// 정수
    Integer(i64),
    /// 실수
    Float(f64),
    /// 문자열
    String(String),
    /// 불리언
    Boolean(bool),
    /// Null
    Null,
    /// 중첩 객체
    Object(HashMap<String, AttributeValue>),
    /// 배열
    Array(Vec<AttributeValue>),
}

impl AttributeValue {
    /// 숫자로 변환 시도
    ///
    /// # Precision Warning
    /// When converting `Integer(i64)` to `f64`, precision loss may occur
    /// for values > 2^53 (9,007,199,254,740,992). This is a limitation of
    /// IEEE 754 double-precision floating point.
    ///
    /// For most JMX metrics (memory bytes, counts, etc.), this is not an issue.
    /// But for extremely large counters, consider tracking the original type.
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            AttributeValue::Integer(i) => {
                // Warn if precision loss is possible (|i| > 2^53)
                if i.abs() > (1i64 << 53) {
                    tracing::warn!(
                        value = i,
                        "Large integer may lose precision when converted to f64"
                    );
                }
                Some(*i as f64)
            }
            AttributeValue::Float(f) => Some(*f),
            AttributeValue::String(s) => s.parse().ok(),
            _ => None,
        }
    }

    /// 문자열로 변환
    pub fn as_string(&self) -> Option<String> {
        match self {
            AttributeValue::String(s) => Some(s.clone()),
            AttributeValue::Integer(i) => Some(i.to_string()),
            AttributeValue::Float(f) => Some(f.to_string()),
            AttributeValue::Boolean(b) => Some(b.to_string()),
            _ => None,
        }
    }
}

/// 단일 응답 파싱
pub fn parse_response(json: &str) -> CollectResult<JolokiaResponse> {
    let raw: RawJolokiaResponse = serde_json::from_str(json)
        .map_err(|e| CollectorError::JsonParse(e.to_string()))?;

    convert_raw_response(raw)
}

/// Bulk 응답 파싱
pub fn parse_bulk_response(json: &str) -> CollectResult<Vec<JolokiaResponse>> {
    let raw_responses: Vec<RawJolokiaResponse> = serde_json::from_str(json)
        .map_err(|e| CollectorError::JsonParse(e.to_string()))?;

    raw_responses
        .into_iter()
        .map(convert_raw_response)
        .collect()
}

// 내부 파싱용 구조체
#[derive(Deserialize)]
struct RawJolokiaResponse {
    request: RequestInfo,
    value: Option<Value>,
    status: u16,
    #[serde(default)]
    timestamp: u64,
    error: Option<String>,
    error_type: Option<String>,
}

fn convert_raw_response(raw: RawJolokiaResponse) -> CollectResult<JolokiaResponse> {
    // 에러 응답 처리
    if raw.status != 200 {
        return Ok(JolokiaResponse {
            request: raw.request,
            value: MBeanValue::Null,
            status: raw.status,
            timestamp: raw.timestamp,
            error: raw.error,
            error_type: raw.error_type,
        });
    }

    let value = match raw.value {
        Some(v) => parse_mbean_value(v)?,
        None => MBeanValue::Null,
    };

    Ok(JolokiaResponse {
        request: raw.request,
        value,
        status: raw.status,
        timestamp: raw.timestamp,
        error: raw.error,
        error_type: raw.error_type,
    })
}

fn parse_mbean_value(value: Value) -> CollectResult<MBeanValue> {
    match value {
        Value::Null => Ok(MBeanValue::Null),
        Value::Bool(b) => Ok(MBeanValue::Boolean(b)),
        Value::Number(n) => {
            Ok(MBeanValue::Number(n.as_f64().unwrap_or(0.0)))
        }
        Value::String(s) => Ok(MBeanValue::String(s)),
        Value::Array(arr) => {
            let parsed: Vec<AttributeValue> = arr
                .into_iter()
                .map(parse_attribute_value)
                .collect::<CollectResult<_>>()?;
            Ok(MBeanValue::Array(parsed))
        }
        Value::Object(map) => {
            // 와일드카드 응답인지 확인 (값이 모두 객체이고 MBean ObjectName 형태)
            let is_wildcard = map.iter().all(|(k, v)| {
                k.contains(':') && k.contains('=') && v.is_object()
            });

            if is_wildcard && !map.is_empty() {
                let mut result = HashMap::new();
                for (mbean_name, attrs) in map {
                    if let Value::Object(attr_map) = attrs {
                        let parsed_attrs: HashMap<String, AttributeValue> = attr_map
                            .into_iter()
                            .map(|(k, v)| Ok((k, parse_attribute_value(v)?)))
                            .collect::<CollectResult<_>>()?;
                        result.insert(mbean_name, parsed_attrs);
                    }
                }
                Ok(MBeanValue::Wildcard(result))
            } else {
                // 일반 CompositeData
                let parsed: HashMap<String, AttributeValue> = map
                    .into_iter()
                    .map(|(k, v)| Ok((k, parse_attribute_value(v)?)))
                    .collect::<CollectResult<_>>()?;
                Ok(MBeanValue::Composite(parsed))
            }
        }
    }
}

fn parse_attribute_value(value: Value) -> CollectResult<AttributeValue> {
    match value {
        Value::Null => Ok(AttributeValue::Null),
        Value::Bool(b) => Ok(AttributeValue::Boolean(b)),
        Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Ok(AttributeValue::Integer(i))
            } else {
                Ok(AttributeValue::Float(n.as_f64().unwrap_or(0.0)))
            }
        }
        Value::String(s) => Ok(AttributeValue::String(s)),
        Value::Array(arr) => {
            let parsed: Vec<AttributeValue> = arr
                .into_iter()
                .map(parse_attribute_value)
                .collect::<CollectResult<_>>()?;
            Ok(AttributeValue::Array(parsed))
        }
        Value::Object(map) => {
            let parsed: HashMap<String, AttributeValue> = map
                .into_iter()
                .map(|(k, v)| Ok((k, parse_attribute_value(v)?)))
                .collect::<CollectResult<_>>()?;
            Ok(AttributeValue::Object(parsed))
        }
    }
}

/// 값 추출 유틸리티 - Prometheus 메트릭용
impl MBeanValue {
    /// Composite 값에서 특정 키의 숫자 추출
    pub fn get_composite_number(&self, key: &str) -> Option<f64> {
        match self {
            MBeanValue::Composite(map) => {
                map.get(key).and_then(|v| v.as_f64())
            }
            _ => None,
        }
    }

    /// 단순 숫자 값 추출
    pub fn as_number(&self) -> Option<f64> {
        match self {
            MBeanValue::Number(n) => Some(*n),
            _ => None,
        }
    }

    /// 모든 숫자 값을 (이름, 값) 쌍으로 평탄화
    pub fn flatten_numbers(&self) -> Vec<(String, f64)> {
        let mut result = Vec::new();
        self.flatten_numbers_inner("", &mut result);
        result
    }

    fn flatten_numbers_inner(&self, prefix: &str, result: &mut Vec<(String, f64)>) {
        match self {
            MBeanValue::Number(n) => {
                let name = if prefix.is_empty() {
                    "value".to_string()
                } else {
                    prefix.to_string()
                };
                result.push((name, *n));
            }
            MBeanValue::Composite(map) => {
                for (key, value) in map {
                    let new_prefix = if prefix.is_empty() {
                        key.clone()
                    } else {
                        format!("{}_{}", prefix, key)
                    };
                    if let Some(n) = value.as_f64() {
                        result.push((new_prefix, n));
                    }
                }
            }
            _ => {}
        }
    }
}
```

---

## 5. 데이터 구조 설계 (Data Structure Design)

### 5.1 핵심 타입 다이어그램

```
JolokiaResponse
├── request: RequestInfo
│   ├── mbean: String          # "java.lang:type=Memory"
│   ├── attribute: Option<Value>
│   └── request_type: String   # "read"
│
├── value: MBeanValue
│   ├── Number(f64)            # 단순 숫자
│   ├── String(String)         # 문자열
│   ├── Boolean(bool)          # 불리언
│   ├── Null                   # null
│   ├── Composite(HashMap)     # CompositeData
│   │   └── "used" -> AttributeValue::Integer(52428800)
│   ├── Array(Vec)             # TabularData
│   └── Wildcard(HashMap)      # 와일드카드 결과
│       └── "java.lang:type=GC,name=G1" -> HashMap
│
├── status: u16                # 200, 404 등
├── timestamp: u64             # Unix epoch
├── error: Option<String>      # 에러 메시지
└── error_type: Option<String> # 예외 타입
```

### 5.2 JMX 타입 매핑

| JMX 타입 | Jolokia JSON | Rust 타입 |
|----------|--------------|-----------|
| `int`, `long` | `number` | `AttributeValue::Integer(i64)` |
| `float`, `double` | `number` | `AttributeValue::Float(f64)` |
| `String` | `string` | `AttributeValue::String(String)` |
| `boolean` | `boolean` | `AttributeValue::Boolean(bool)` |
| `CompositeData` | `object` | `MBeanValue::Composite` |
| `TabularData` | `array` | `MBeanValue::Array` |
| `ObjectName[]` | `array` | `AttributeValue::Array` |

### 5.3 MBean ObjectName 파서

```rust
/// MBean ObjectName 구조
#[derive(Debug, Clone, PartialEq)]
pub struct ObjectName {
    /// 도메인 (예: "java.lang")
    pub domain: String,
    /// 속성 (예: {"type": "Memory"})
    pub properties: HashMap<String, String>,
}

impl ObjectName {
    /// ObjectName 문자열 파싱
    ///
    /// # Example
    /// ```
    /// let name = ObjectName::parse("java.lang:type=Memory")?;
    /// assert_eq!(name.domain, "java.lang");
    /// assert_eq!(name.properties.get("type"), Some(&"Memory".to_string()));
    /// ```
    ///
    /// # Limitations
    /// - **Quoted keys/values are NOT fully supported**: Values containing `,` or `=`
    ///   wrapped in quotes (e.g., `name="foo,bar"`) will be parsed incorrectly.
    /// - For full JMX ObjectName compliance, a proper parser with quote handling
    ///   is needed. This is a known limitation for Phase 2.
    ///
    /// # TODO (Future Enhancement)
    /// - Support quoted values: `name="value with, special=chars"`
    /// - Support escaped quotes: `name="value with \"quotes\""`
    pub fn parse(s: &str) -> CollectResult<Self> {
        let parts: Vec<&str> = s.splitn(2, ':').collect();
        if parts.len() != 2 {
            return Err(CollectorError::InvalidObjectName(s.to_string()));
        }

        let domain = parts[0].to_string();
        let mut properties = HashMap::new();

        // Simple parser - does NOT handle quoted values correctly
        // e.g., name="foo,bar" will be split incorrectly
        for prop in parts[1].split(',') {
            let kv: Vec<&str> = prop.splitn(2, '=').collect();
            if kv.len() == 2 {
                properties.insert(kv[0].to_string(), kv[1].to_string());
            }
        }

        Ok(Self { domain, properties })
    }

    /// Prometheus 라벨용 문자열 생성
    pub fn to_label_string(&self) -> String {
        let props: Vec<String> = self
            .properties
            .iter()
            .map(|(k, v)| format!("{}=\"{}\"", k, v))
            .collect();
        format!("{}:{}", self.domain, props.join(","))
    }
}
```

---

## 6. HTTP 클라이언트 구현 (HTTP Client Implementation)

### 6.1 Connection Pooling

```rust
// Reqwest 기본 Connection Pool 설정
let client = ClientBuilder::new()
    // 호스트당 최대 유휴 연결 수
    .pool_max_idle_per_host(10)
    // 유휴 연결 유지 시간
    .pool_idle_timeout(Duration::from_secs(30))
    // HTTP/2 지원 (가능한 경우)
    .http2_prior_knowledge()
    .build()?;
```

### 6.2 Timeout Handling

```rust
/// 타임아웃 계층 구조
pub struct TimeoutConfig {
    /// 연결 타임아웃
    pub connect_timeout: Duration,
    /// 요청 전체 타임아웃
    pub request_timeout: Duration,
    /// 읽기 타임아웃
    pub read_timeout: Duration,
}

impl Default for TimeoutConfig {
    fn default() -> Self {
        Self {
            connect_timeout: Duration::from_secs(2),
            request_timeout: Duration::from_secs(5),
            read_timeout: Duration::from_secs(5),
        }
    }
}

// 클라이언트에 적용
let client = ClientBuilder::new()
    .connect_timeout(config.connect_timeout)
    .timeout(config.request_timeout)
    .read_timeout(config.read_timeout)
    .build()?;
```

### 6.3 Retry Logic

```rust
use std::time::Duration;
use tokio::time::sleep;

/// 재시도 설정
pub struct RetryConfig {
    pub max_retries: u32,
    pub initial_delay: Duration,
    pub max_delay: Duration,
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
    /// 재시도 로직이 포함된 요청
    async fn request_with_retry<F, Fut, T>(
        &self,
        operation: F,
        config: &RetryConfig,
    ) -> CollectResult<T>
    where
        F: Fn() -> Fut,
        Fut: std::future::Future<Output = CollectResult<T>>,
    {
        let mut delay = config.initial_delay;
        let mut last_error = None;

        for attempt in 0..=config.max_retries {
            match operation().await {
                Ok(result) => return Ok(result),
                Err(e) => {
                    // 재시도 불가능한 에러인 경우 즉시 반환
                    if !e.is_retryable() {
                        return Err(e);
                    }

                    last_error = Some(e);

                    if attempt < config.max_retries {
                        warn!(
                            attempt = attempt + 1,
                            max = config.max_retries,
                            delay_ms = delay.as_millis(),
                            "Request failed, retrying"
                        );
                        sleep(delay).await;
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
}
```

---

## 7. 에러 처리 (Error Handling)

### 7.1 CollectorError 정의

```rust
// src/error.rs

use thiserror::Error;

/// Collector 모듈 에러 타입
#[derive(Error, Debug)]
pub enum CollectorError {
    /// HTTP 클라이언트 초기화 실패
    #[error("Failed to initialize HTTP client: {0}")]
    HttpClientInit(#[source] reqwest::Error),

    /// HTTP 요청 실패
    #[error("HTTP request failed: {0}")]
    HttpRequest(#[source] reqwest::Error),

    /// HTTP 응답 읽기 실패
    #[error("Failed to read HTTP response: {0}")]
    HttpResponse(#[source] reqwest::Error),

    /// HTTP 상태 코드 에러
    #[error("HTTP error status: {0}")]
    HttpStatus(u16),

    /// JSON 파싱 에러
    #[error("JSON parse error: {0}")]
    JsonParse(String),

    /// Jolokia 에러 응답
    #[error("Jolokia error (status {status}): {message}")]
    JolokiaError {
        status: u16,
        message: String,
    },

    /// MBean을 찾을 수 없음
    #[error("MBean not found: {0}")]
    MBeanNotFound(String),

    /// 잘못된 ObjectName
    #[error("Invalid ObjectName: {0}")]
    InvalidObjectName(String),

    /// 타임아웃
    #[error("Request timed out after {0}ms")]
    Timeout(u64),

    /// 연결 실패
    #[error("Connection failed: {0}")]
    ConnectionFailed(String),

    /// 최대 재시도 초과
    #[error("Maximum retries exceeded")]
    MaxRetriesExceeded,

    /// 인증 실패
    #[error("Authentication failed")]
    AuthenticationFailed,
}

impl CollectorError {
    /// 재시도 가능한 에러인지 확인
    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            CollectorError::HttpRequest(_)
                | CollectorError::HttpResponse(_)
                | CollectorError::Timeout(_)
                | CollectorError::ConnectionFailed(_)
                | CollectorError::HttpStatus(500..=599)
        )
    }

    /// HTTP 상태 코드 추출
    pub fn http_status(&self) -> Option<u16> {
        match self {
            CollectorError::HttpStatus(code) => Some(*code),
            CollectorError::JolokiaError { status, .. } => Some(*status),
            _ => None,
        }
    }
}

// reqwest 에러를 CollectorError로 변환하는 헬퍼
impl From<reqwest::Error> for CollectorError {
    fn from(err: reqwest::Error) -> Self {
        if err.is_timeout() {
            CollectorError::Timeout(5000) // 기본값
        } else if err.is_connect() {
            CollectorError::ConnectionFailed(err.to_string())
        } else if err.is_request() {
            CollectorError::HttpRequest(err)
        } else {
            CollectorError::HttpResponse(err)
        }
    }
}
```

### 7.2 에러 처리 예시

```rust
use tracing::{error, warn};

impl JolokiaClient {
    pub async fn collect_with_fallback(
        &self,
        mbeans: &[String],
    ) -> Vec<(String, CollectResult<JolokiaResponse>)> {
        let mut results = Vec::new();

        for mbean in mbeans {
            let result = self.read_mbean(mbean, None).await;

            match &result {
                Ok(response) if response.status == 200 => {
                    // 성공
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
                    error!(
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
```

---

## 8. 테스트 계획 (Test Plan)

### 8.1 단위 테스트 (Unit Tests)

#### parser.rs 테스트

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_number_response() {
        let json = r#"{
            "request": {
                "mbean": "java.lang:type=Threading",
                "attribute": "ThreadCount",
                "type": "read"
            },
            "value": 42,
            "timestamp": 1609459200,
            "status": 200
        }"#;

        let response = parse_response(json).unwrap();
        assert_eq!(response.status, 200);
        assert!(matches!(response.value, MBeanValue::Number(n) if n == 42.0));
    }

    #[test]
    fn test_parse_composite_response() {
        let json = r#"{
            "request": {
                "mbean": "java.lang:type=Memory",
                "attribute": "HeapMemoryUsage",
                "type": "read"
            },
            "value": {
                "init": 268435456,
                "committed": 268435456,
                "max": 4294967296,
                "used": 52428800
            },
            "timestamp": 1609459200,
            "status": 200
        }"#;

        let response = parse_response(json).unwrap();
        assert_eq!(response.status, 200);

        if let MBeanValue::Composite(map) = &response.value {
            assert_eq!(map.get("used").and_then(|v| v.as_f64()), Some(52428800.0));
            assert_eq!(map.get("max").and_then(|v| v.as_f64()), Some(4294967296.0));
        } else {
            panic!("Expected Composite value");
        }
    }

    #[test]
    fn test_parse_error_response() {
        let json = r#"{
            "request": {
                "mbean": "invalid:type=NotFound",
                "type": "read"
            },
            "error_type": "javax.management.InstanceNotFoundException",
            "error": "No MBean found",
            "status": 404
        }"#;

        let response = parse_response(json).unwrap();
        assert_eq!(response.status, 404);
        assert!(response.error.is_some());
        assert_eq!(
            response.error_type,
            Some("javax.management.InstanceNotFoundException".to_string())
        );
    }

    #[test]
    fn test_parse_bulk_response() {
        let json = r#"[
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
        ]"#;

        let responses = parse_bulk_response(json).unwrap();
        assert_eq!(responses.len(), 2);
        assert_eq!(responses[0].status, 200);
        assert_eq!(responses[1].status, 200);
    }

    #[test]
    fn test_parse_wildcard_response() {
        let json = r#"{
            "request": {
                "mbean": "java.lang:type=GarbageCollector,name=*",
                "type": "read"
            },
            "value": {
                "java.lang:type=GarbageCollector,name=G1 Young Generation": {
                    "CollectionCount": 42,
                    "CollectionTime": 1234
                },
                "java.lang:type=GarbageCollector,name=G1 Old Generation": {
                    "CollectionCount": 5,
                    "CollectionTime": 567
                }
            },
            "timestamp": 1609459200,
            "status": 200
        }"#;

        let response = parse_response(json).unwrap();
        if let MBeanValue::Wildcard(map) = &response.value {
            assert_eq!(map.len(), 2);
            assert!(map.contains_key("java.lang:type=GarbageCollector,name=G1 Young Generation"));
        } else {
            panic!("Expected Wildcard value");
        }
    }

    #[test]
    fn test_object_name_parse() {
        let name = ObjectName::parse("java.lang:type=Memory").unwrap();
        assert_eq!(name.domain, "java.lang");
        assert_eq!(name.properties.get("type"), Some(&"Memory".to_string()));

        let name2 = ObjectName::parse("java.lang:type=GarbageCollector,name=G1").unwrap();
        assert_eq!(name2.properties.get("name"), Some(&"G1".to_string()));
    }

    #[test]
    fn test_flatten_numbers() {
        let value = MBeanValue::Composite(HashMap::from([
            ("used".to_string(), AttributeValue::Integer(1000)),
            ("max".to_string(), AttributeValue::Integer(2000)),
            ("name".to_string(), AttributeValue::String("test".to_string())),
        ]));

        let flattened = value.flatten_numbers();
        assert_eq!(flattened.len(), 2);
    }
}
```

### 8.2 통합 테스트 (Integration Tests with wiremock)

```rust
// tests/collector_integration.rs

use wiremock::{MockServer, Mock, ResponseTemplate};
use wiremock::matchers::{method, path, body_json};
use serde_json::json;

use rjmx_exporter::collector::JolokiaClient;

#[tokio::test]
async fn test_read_mbean_success() {
    // Mock 서버 시작
    let mock_server = MockServer::start().await;

    // 예상 요청/응답 설정
    Mock::given(method("POST"))
        .and(path("/jolokia"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "request": {
                "mbean": "java.lang:type=Memory",
                "type": "read"
            },
            "value": {
                "HeapMemoryUsage": {
                    "used": 52428800,
                    "max": 4294967296
                }
            },
            "timestamp": 1609459200,
            "status": 200
        })))
        .mount(&mock_server)
        .await;

    // 클라이언트 생성 및 요청
    let client = JolokiaClient::new(&mock_server.uri(), 5000).unwrap();
    let response = client.read_mbean("java.lang:type=Memory", None).await.unwrap();

    assert_eq!(response.status, 200);
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

    let client = JolokiaClient::new(&mock_server.uri(), 5000).unwrap();
    let responses = client
        .read_mbeans_bulk(&[
            ("java.lang:type=Threading", None),
            ("java.lang:type=Memory", None),
        ])
        .await
        .unwrap();

    assert_eq!(responses.len(), 2);
}

#[tokio::test]
async fn test_timeout_handling() {
    let mock_server = MockServer::start().await;

    // 지연 응답 설정
    Mock::given(method("POST"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_delay(std::time::Duration::from_secs(10))
        )
        .mount(&mock_server)
        .await;

    // 짧은 타임아웃으로 클라이언트 생성
    let client = JolokiaClient::new(&mock_server.uri(), 100).unwrap();
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

    let client = JolokiaClient::new(&mock_server.uri(), 5000).unwrap();
    let response = client.read_mbean("invalid:type=NotFound", None).await.unwrap();

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
        .expect(10)  // 10번 호출 예상
        .mount(&mock_server)
        .await;

    let client = JolokiaClient::new(&mock_server.uri(), 5000).unwrap();

    // 동일 클라이언트로 여러 번 요청
    for _ in 0..10 {
        let result = client.read_mbean("test:type=Test", None).await;
        assert!(result.is_ok());
    }
}
```

### 8.3 벤치마크 테스트

```rust
// benches/collector_bench.rs

use criterion::{criterion_group, criterion_main, Criterion, BenchmarkId};
use rjmx_exporter::collector::parser::parse_response;

fn benchmark_parse_response(c: &mut Criterion) {
    let simple_json = r#"{
        "request": {"mbean": "java.lang:type=Threading", "type": "read"},
        "value": 42,
        "timestamp": 1609459200,
        "status": 200
    }"#;

    let composite_json = r#"{
        "request": {"mbean": "java.lang:type=Memory", "type": "read"},
        "value": {
            "init": 268435456,
            "committed": 268435456,
            "max": 4294967296,
            "used": 52428800
        },
        "timestamp": 1609459200,
        "status": 200
    }"#;

    let mut group = c.benchmark_group("parse_response");

    group.bench_with_input(
        BenchmarkId::new("simple", "number"),
        &simple_json,
        |b, json| b.iter(|| parse_response(json)),
    );

    group.bench_with_input(
        BenchmarkId::new("composite", "memory"),
        &composite_json,
        |b, json| b.iter(|| parse_response(json)),
    );

    group.finish();
}

criterion_group!(benches, benchmark_parse_response);
criterion_main!(benches);
```

---

## 9. 완료 기준 (Definition of Done)

### 9.1 기능 완료 체크리스트

- [ ] **collector/mod.rs**
  - [ ] 모듈 공개 인터페이스 정의
  - [ ] `CollectConfig` 구조체 구현
  - [ ] 문서화 (rustdoc)

- [ ] **collector/client.rs**
  - [ ] `JolokiaClient::new()` 구현
  - [ ] `read_mbean()` 단일 조회 구현
  - [ ] `read_mbeans_bulk()` 일괄 조회 구현
  - [ ] `search_mbeans()` 검색 구현
  - [ ] Connection pooling 설정
  - [ ] Timeout 처리
  - [ ] Basic Auth 지원

- [ ] **collector/parser.rs**
  - [ ] `JolokiaResponse` 구조체 정의
  - [ ] `MBeanValue` enum 정의
  - [ ] `AttributeValue` enum 정의
  - [ ] `parse_response()` 구현
  - [ ] `parse_bulk_response()` 구현
  - [ ] `ObjectName` 파서 구현
  - [ ] Composite/Wildcard 값 처리

- [ ] **error.rs**
  - [ ] `CollectorError` enum 정의
  - [ ] `is_retryable()` 구현
  - [ ] `From` trait 구현

### 9.2 품질 기준

| 항목 | 기준 |
|------|------|
| 테스트 커버리지 | 80% 이상 |
| Clippy 경고 | 0개 |
| 문서화 | 모든 pub 항목 |
| 에러 처리 | `unwrap()` 사용 금지 |

### 9.3 성능 기준

| 메트릭 | 목표 | 측정 방법 |
|--------|------|-----------|
| 단일 파싱 | < 100μs | `criterion` 벤치마크 |
| Bulk 파싱 (10개) | < 1ms | `criterion` 벤치마크 |
| 메모리 할당 | 응답 크기의 2배 이하 | `dhat` 프로파일링 |

### 9.4 통합 테스트 기준

- [ ] Mock 서버 기반 통합 테스트 통과
- [ ] 실제 Jolokia 엔드포인트 연동 테스트 (선택)
- [ ] 에러 시나리오 테스트 (타임아웃, 404, 500)

---

## 부록 A: Jolokia 응답 예시

### A.1 Memory MBean

```json
{
  "request": {
    "mbean": "java.lang:type=Memory",
    "type": "read"
  },
  "value": {
    "HeapMemoryUsage": {
      "init": 268435456,
      "committed": 268435456,
      "max": 4294967296,
      "used": 52428800
    },
    "NonHeapMemoryUsage": {
      "init": 2555904,
      "committed": 71827456,
      "max": -1,
      "used": 68891240
    },
    "ObjectPendingFinalizationCount": 0,
    "Verbose": false
  },
  "timestamp": 1609459200,
  "status": 200
}
```

### A.2 GarbageCollector (와일드카드)

```json
{
  "request": {
    "mbean": "java.lang:type=GarbageCollector,name=*",
    "type": "read"
  },
  "value": {
    "java.lang:type=GarbageCollector,name=G1 Young Generation": {
      "CollectionCount": 42,
      "CollectionTime": 1234,
      "Name": "G1 Young Generation",
      "Valid": true
    },
    "java.lang:type=GarbageCollector,name=G1 Old Generation": {
      "CollectionCount": 5,
      "CollectionTime": 567,
      "Name": "G1 Old Generation",
      "Valid": true
    }
  },
  "timestamp": 1609459200,
  "status": 200
}
```

### A.3 ThreadInfo (TabularData)

```json
{
  "request": {
    "mbean": "java.lang:type=Threading",
    "attribute": "AllThreadIds",
    "type": "read"
  },
  "value": [1, 2, 3, 4, 5, 6, 7, 8],
  "timestamp": 1609459200,
  "status": 200
}
```

---

## 부록 B: 참고 자료

- [Jolokia Protocol Reference](https://jolokia.org/reference/html/protocol.html)
- [Reqwest Documentation](https://docs.rs/reqwest)
- [Serde JSON Documentation](https://docs.rs/serde_json)
- [Wiremock for Rust](https://docs.rs/wiremock)
- [Prometheus Exposition Format](https://prometheus.io/docs/instrumenting/exposition_formats/)
