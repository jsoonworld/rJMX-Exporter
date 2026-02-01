//! Jolokia JSON 응답 파서
//!
//! Jolokia API 응답을 파싱하여 내부 데이터 구조로 변환합니다.

use serde::Deserialize;
use serde_json::Value;
use std::collections::HashMap;

use crate::error::CollectorError;

/// Collector 작업 결과 타입
pub type CollectResult<T> = Result<T, CollectorError>;

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
    /// for values > 2^53 (9,007,199,254,740,992).
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            AttributeValue::Integer(i) => {
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
    let raw: RawJolokiaResponse =
        serde_json::from_str(json).map_err(|e| CollectorError::JsonParse(e.to_string()))?;

    convert_raw_response(raw)
}

/// Bulk 응답 파싱
pub fn parse_bulk_response(json: &str) -> CollectResult<Vec<JolokiaResponse>> {
    let raw_responses: Vec<RawJolokiaResponse> =
        serde_json::from_str(json).map_err(|e| CollectorError::JsonParse(e.to_string()))?;

    raw_responses
        .into_iter()
        .map(convert_raw_response)
        .collect()
}

/// 내부 파싱용 구조체
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
            let f = n.as_f64().ok_or_else(|| {
                CollectorError::JsonParse(format!("Number {} cannot be represented as f64", n))
            })?;
            Ok(MBeanValue::Number(f))
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
            let is_wildcard = map
                .iter()
                .all(|(k, v)| k.contains(':') && k.contains('=') && v.is_object());

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
                Ok(AttributeValue::Float(n.as_f64().ok_or_else(|| {
                    CollectorError::JsonParse(format!("Number {} cannot be represented as f64", n))
                })?))
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
            MBeanValue::Composite(map) => map.get(key).and_then(|v| v.as_f64()),
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
    /// # Limitations
    /// - Quoted keys/values are NOT fully supported
    pub fn parse(s: &str) -> CollectResult<Self> {
        let parts: Vec<&str> = s.splitn(2, ':').collect();
        if parts.len() != 2 {
            return Err(CollectorError::InvalidObjectName(s.to_string()));
        }

        let domain = parts[0].to_string();
        let mut properties = HashMap::new();

        for prop in parts[1].split(',') {
            let kv: Vec<&str> = prop.splitn(2, '=').collect();
            if kv.len() == 2 {
                properties.insert(kv[0].to_string(), kv[1].to_string());
            }
        }

        Ok(Self { domain, properties })
    }

    /// Prometheus 라벨용 문자열 생성
    ///
    /// Properties are sorted alphabetically by key to ensure deterministic output.
    pub fn to_label_string(&self) -> String {
        let mut props: Vec<(&String, &String)> = self.properties.iter().collect();
        props.sort_by_key(|(k, _)| *k);
        let prop_strs: Vec<String> = props
            .iter()
            .map(|(k, v)| format!("{}=\"{}\"", k, v))
            .collect();
        format!("{}:{}", self.domain, prop_strs.join(","))
    }
}

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
        assert!(matches!(response.value, MBeanValue::Number(n) if (n - 42.0).abs() < f64::EPSILON));
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
            (
                "name".to_string(),
                AttributeValue::String("test".to_string()),
            ),
        ]));

        let flattened = value.flatten_numbers();
        assert_eq!(flattened.len(), 2);
    }
}
