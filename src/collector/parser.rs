//! Jolokia JSON response parser
//!
//! Parses Jolokia API responses and converts them to internal data structures.

use serde::Deserialize;
use serde_json::Value;
use std::collections::HashMap;

use crate::error::CollectorError;

/// Collector operation result type
pub type CollectResult<T> = Result<T, CollectorError>;

/// Jolokia API response struct
#[derive(Debug, Clone)]
pub struct JolokiaResponse {
    /// Request information
    pub request: RequestInfo,
    /// Response value
    pub value: MBeanValue,
    /// Response status code
    pub status: u16,
    /// Timestamp (Unix epoch)
    pub timestamp: u64,
    /// Error message (on failure)
    pub error: Option<String>,
    /// Error type (on failure)
    pub error_type: Option<String>,
}

/// Request information
#[derive(Debug, Clone, Deserialize)]
pub struct RequestInfo {
    /// MBean ObjectName
    pub mbean: String,
    /// Queried attributes (single or multiple)
    #[serde(default)]
    pub attribute: Option<Value>,
    /// Request type
    #[serde(rename = "type")]
    pub request_type: String,
}

/// MBean value - supports various formats
#[derive(Debug, Clone)]
pub enum MBeanValue {
    /// Simple numeric value
    Number(f64),
    /// String value
    String(String),
    /// Boolean value
    Boolean(bool),
    /// Null value
    Null,
    /// Composite object (CompositeData)
    Composite(HashMap<String, AttributeValue>),
    /// Array
    Array(Vec<AttributeValue>),
    /// Wildcard result (MBean ObjectName -> attribute map)
    Wildcard(HashMap<String, HashMap<String, AttributeValue>>),
}

/// Individual attribute value
#[derive(Debug, Clone)]
pub enum AttributeValue {
    /// Integer
    Integer(i64),
    /// Float
    Float(f64),
    /// String
    String(String),
    /// Boolean
    Boolean(bool),
    /// Null
    Null,
    /// Nested object
    Object(HashMap<String, AttributeValue>),
    /// Array
    Array(Vec<AttributeValue>),
}

impl AttributeValue {
    /// Try to convert to number
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

    /// Convert to string
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

/// Parse a single response
pub fn parse_response(json: &str) -> CollectResult<JolokiaResponse> {
    let raw: RawJolokiaResponse =
        serde_json::from_str(json).map_err(|e| CollectorError::JsonParse(e.to_string()))?;

    convert_raw_response(raw)
}

/// Parse bulk response
pub fn parse_bulk_response(json: &str) -> CollectResult<Vec<JolokiaResponse>> {
    let raw_responses: Vec<RawJolokiaResponse> =
        serde_json::from_str(json).map_err(|e| CollectorError::JsonParse(e.to_string()))?;

    raw_responses
        .into_iter()
        .map(convert_raw_response)
        .collect()
}

/// Internal struct for parsing
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
    // Handle error response
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
            // Check if this is a wildcard response (all values are objects and keys are MBean ObjectNames)
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
                // Regular CompositeData
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

/// Value extraction utilities for Prometheus metrics
impl MBeanValue {
    /// Extract a number from a composite value by key
    pub fn get_composite_number(&self, key: &str) -> Option<f64> {
        match self {
            MBeanValue::Composite(map) => map.get(key).and_then(|v| v.as_f64()),
            _ => None,
        }
    }

    /// Extract simple numeric value
    pub fn as_number(&self) -> Option<f64> {
        match self {
            MBeanValue::Number(n) => Some(*n),
            _ => None,
        }
    }

    /// Flatten all numeric values into (name, value) pairs
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

/// MBean ObjectName structure
#[derive(Debug, Clone, PartialEq)]
pub struct ObjectName {
    /// Domain (e.g., "java.lang")
    pub domain: String,
    /// Properties (e.g., {"type": "Memory"})
    pub properties: HashMap<String, String>,
}

impl ObjectName {
    /// Parse ObjectName string
    ///
    /// # Limitations
    /// - Quoted keys/values are NOT fully supported
    ///
    /// # Errors
    /// Returns `InvalidObjectName` if:
    /// - Missing domain/properties separator (':')
    /// - Any property segment is not in key=value format
    /// - No properties are defined
    pub fn parse(s: &str) -> CollectResult<Self> {
        let parts: Vec<&str> = s.splitn(2, ':').collect();
        if parts.len() != 2 {
            return Err(CollectorError::InvalidObjectName(s.to_string()));
        }

        let domain = parts[0].trim().to_string();
        if domain.is_empty() {
            return Err(CollectorError::InvalidObjectName(s.to_string()));
        }

        let mut properties = HashMap::new();

        for prop in parts[1].split(',') {
            let kv: Vec<&str> = prop.splitn(2, '=').collect();
            if kv.len() != 2 {
                return Err(CollectorError::InvalidObjectName(s.to_string()));
            }
            let key = kv[0].trim();
            let value = kv[1].trim();
            if key.is_empty() {
                return Err(CollectorError::InvalidObjectName(s.to_string()));
            }
            properties.insert(key.to_string(), value.to_string());
        }

        if properties.is_empty() {
            return Err(CollectorError::InvalidObjectName(s.to_string()));
        }

        Ok(Self { domain, properties })
    }

    /// Generate string for Prometheus labels
    ///
    /// Properties are sorted alphabetically by key to ensure deterministic output.
    /// Label values are escaped according to Prometheus text format rules.
    pub fn to_label_string(&self) -> String {
        let mut props: Vec<(&String, &String)> = self.properties.iter().collect();
        props.sort_by_key(|(k, _)| *k);
        let prop_strs: Vec<String> = props
            .iter()
            .map(|(k, v)| format!("{}=\"{}\"", k, Self::escape_label_value(v)))
            .collect();
        format!("{}:{}", self.domain, prop_strs.join(","))
    }

    /// Escape a label value for Prometheus text format
    ///
    /// Prometheus requires escaping of: backslash (\), double-quote ("), and newline (\n)
    fn escape_label_value(s: &str) -> String {
        let mut result = String::with_capacity(s.len());
        for c in s.chars() {
            match c {
                '\\' => result.push_str("\\\\"),
                '"' => result.push_str("\\\""),
                '\n' => result.push_str("\\n"),
                _ => result.push(c),
            }
        }
        result
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
    fn test_object_name_parse_invalid() {
        // Missing colon separator
        assert!(ObjectName::parse("java.lang").is_err());

        // Empty domain
        assert!(ObjectName::parse(":type=Memory").is_err());

        // Missing property value (no equals sign)
        assert!(ObjectName::parse("java.lang:type").is_err());

        // Empty key
        assert!(ObjectName::parse("java.lang:=Memory").is_err());

        // Empty properties section
        assert!(ObjectName::parse("java.lang:").is_err());
    }

    #[test]
    fn test_object_name_parse_with_whitespace() {
        // Whitespace should be trimmed
        let name = ObjectName::parse(" java.lang : type = Memory ").unwrap();
        assert_eq!(name.domain, "java.lang");
        assert_eq!(name.properties.get("type"), Some(&"Memory".to_string()));
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

    #[test]
    fn test_label_escaping() {
        // Test escaping of backslash
        assert_eq!(ObjectName::escape_label_value("a\\b"), "a\\\\b");

        // Test escaping of double-quote
        assert_eq!(ObjectName::escape_label_value("a\"b"), "a\\\"b");

        // Test escaping of newline
        assert_eq!(ObjectName::escape_label_value("a\nb"), "a\\nb");

        // Test combined escaping
        assert_eq!(
            ObjectName::escape_label_value("path\\to\\\"file\"\nend"),
            "path\\\\to\\\\\\\"file\\\"\\nend"
        );

        // Test no escaping needed
        assert_eq!(
            ObjectName::escape_label_value("normal_value"),
            "normal_value"
        );
    }

    #[test]
    fn test_to_label_string_with_special_chars() {
        let name = ObjectName {
            domain: "java.lang".to_string(),
            properties: HashMap::from([
                ("type".to_string(), "GarbageCollector".to_string()),
                ("name".to_string(), "G1 \"Young\" Gen".to_string()),
            ]),
        };

        let label_str = name.to_label_string();
        // Should escape the double quotes in "Young"
        assert!(label_str.contains("name=\"G1 \\\"Young\\\" Gen\""));
    }
}
