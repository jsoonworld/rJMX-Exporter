# Phase 3: Transform Engine 구현 계획서

> **Phase 3** - YAML 규칙 파싱, Regex 기반 메트릭 변환, Prometheus 포맷 출력

---

## 1. 개요 (Overview)

Phase 3는 rJMX-Exporter의 핵심 기능인 **Transform Engine**을 구현한다. 이 엔진은 Jolokia로부터 수집한 MBean 데이터를 jmx_exporter 호환 규칙에 따라 Prometheus 메트릭 포맷으로 변환한다.

### 아키텍처 위치

```
Jolokia (JSON) → Collector → [Transform Engine] → Server (/metrics)
                                    ↑
                              Phase 3 범위
```

### 주요 컴포넌트

| 컴포넌트 | 파일 | 역할 |
|----------|------|------|
| Rule Parser | `transformer/rules.rs` | YAML 규칙 파싱 및 검증 |
| Transform Engine | `transformer/mod.rs` | MBean → Prometheus 변환 |
| Config Updates | `config.rs` | 규칙 설정 통합 |

---

## 2. 목표 (Goals)

### 기능 목표

| 목표 | 설명 | 우선순위 |
|------|------|----------|
| jmx_exporter 호환성 | 기존 설정 파일 최소 수정으로 동작 | P0 |
| 규칙 기반 변환 | pattern, name, type, labels, help 지원 | P0 |
| Capture Group 치환 | `$1`, `$2` 등 동적 메트릭명 생성 | P0 |
| Prometheus 출력 | 표준 exposition format 준수 | P0 |

### 성능 목표

| 메트릭 | 목표값 | 측정 방법 |
|--------|--------|-----------|
| 변환 지연시간 | < 5ms (1000 메트릭) | criterion 벤치마크 |
| 메모리 사용량 | < 2MB (규칙 + 캐시) | 프로파일링 |
| Regex 컴파일 | 시작 시 1회 | OnceCell 사용 |

---

## 3. jmx_exporter 호환성 (jmx_exporter Compatibility)

### 3.1 지원 규칙 옵션

#### P0 (필수 지원)

| 옵션 | 타입 | 설명 | 예시 |
|------|------|------|------|
| `pattern` | String | MBean 매칭 정규식 | `"java.lang<type=Memory>.*"` |
| `name` | String | Prometheus 메트릭명 | `"jvm_memory_$1_bytes"` |
| `type` | Enum | 메트릭 타입 | `gauge`, `counter`, `untyped` |
| `labels` | Map | 라벨 키-값 맵 | `{ area: "$1" }` |
| `help` | String | 메트릭 설명 | `"JVM heap memory"` |

#### P1 (높은 우선순위)

| 옵션 | 타입 | 설명 |
|------|------|------|
| `value` | String | 값 추출 표현식 (기본: 속성값) |
| `valueFactor` | Float | 값 곱셈 인자 (예: ms→s는 0.001) |

#### P2 (중간 우선순위)

| 옵션 | 타입 | 설명 |
|------|------|------|
| `attrNameSnakeCase` | Bool | 속성명 snake_case 변환 |
| `cache` | Bool | 정적 메트릭 캐싱 |

### 3.2 Capture Groups ($1, $2)

jmx_exporter는 정규식 캡처 그룹을 사용하여 동적으로 메트릭명과 라벨을 생성한다.

**jmx_exporter 설정 예시:**

```yaml
rules:
  - pattern: "java.lang<type=Memory><(\\w+)MemoryUsage>(\\w+)"
    name: "jvm_memory_$1_$2_bytes"
    type: gauge
    labels:
      area: "$1"
      metric: "$2"
    help: "JVM $1 memory $2"
```

**입력 MBean:**
```
java.lang<type=Memory><HeapMemoryUsage>used = 123456789
```

**출력 메트릭:**
```
# HELP jvm_memory_heap_used_bytes JVM heap memory used
# TYPE jvm_memory_heap_used_bytes gauge
jvm_memory_heap_used_bytes{area="heap",metric="used"} 1.23456789e+08
```

### 3.3 Regex 호환성 (Java vs Rust)

#### 호환 가능한 패턴

| 기능 | Java 문법 | Rust 문법 | 상태 |
|------|-----------|-----------|------|
| 기본 정규식 | `.*`, `\w+`, `[a-z]+` | 동일 | 호환 |
| 캡처 그룹 | `(\\w+)` | `(\\w+)` | 호환 |
| 비캡처 그룹 | `(?:...)` | `(?:...)` | 호환 |
| 문자 클래스 | `[a-zA-Z0-9_]` | 동일 | 호환 |
| 앵커 | `^`, `$` | 동일 | 호환 |
| 수량자 | `*`, `+`, `?`, `{n,m}` | 동일 | 호환 |

#### 호환되지 않는 패턴

| 기능 | Java 문법 | Rust 대안 | 처리 방안 |
|------|-----------|-----------|-----------|
| Named groups | `(?<name>...)` | `(?P<name>...)` | 자동 변환 |
| Possessive | `++`, `*+` | N/A | 경고 + 일반 수량자로 변환 |
| Atomic groups | `(?>...)` | N/A | 에러 반환 |
| Unicode classes | `\p{javaLowerCase}` | `\p{Ll}` | 매핑 테이블 |
| Lookbehind (가변) | `(?<=.*)` | N/A | 에러 반환 |

#### 자동 변환 규칙

```rust
/// Java regex → Rust regex 변환
fn convert_java_regex(pattern: &str) -> Result<String, RegexError> {
    let mut result = pattern.to_string();

    // Named groups: (?<name>...) → (?P<name>...)
    result = NAMED_GROUP_RE.replace_all(&result, "(?P<$1>$2)").to_string();

    // Possessive quantifiers: ++ → + (경고 로그)
    if POSSESSIVE_RE.is_match(&result) {
        tracing::warn!("Possessive quantifier not supported, using greedy");
        result = POSSESSIVE_RE.replace_all(&result, "$1").to_string();
    }

    Ok(result)
}
```

---

## 4. 구현 항목 (Implementation Items)

### 4.1 transformer/mod.rs

Transform Engine의 메인 모듈로, MBean 데이터를 Prometheus 포맷으로 변환한다.

```rust
//! Transform Engine - MBean to Prometheus metric conversion
//!
//! This module provides the core transformation logic that converts
//! JMX MBean data into Prometheus exposition format.

mod rules;

pub use rules::{Rule, RuleSet, MetricType};

use crate::collector::MBeanData;
use crate::error::TransformError;

/// Transform Engine configuration
#[derive(Debug, Clone)]
pub struct TransformEngine {
    rules: RuleSet,
    lowercase_names: bool,
    lowercase_labels: bool,
}

impl TransformEngine {
    /// Create a new TransformEngine with the given rules
    pub fn new(rules: RuleSet) -> Self {
        Self {
            rules,
            lowercase_names: false,
            lowercase_labels: false,
        }
    }

    /// Transform MBean data into Prometheus metrics
    ///
    /// # Note
    /// A single MBean may produce multiple metrics when:
    /// - CompositeData: Each numeric field becomes a separate metric
    /// - Wildcard query: Each matched MBean produces its own metric(s)
    pub fn transform(&self, mbeans: &[MBeanData]) -> Result<Vec<PrometheusMetric>, TransformError> {
        let mut metrics = Vec::new();

        for mbean in mbeans {
            // Note: transform_mbean returns Vec because Composite/Wildcard
            // can produce multiple metrics from a single MBean
            let mbean_metrics = self.transform_mbean(mbean)?;
            metrics.extend(mbean_metrics);
        }

        Ok(metrics)
    }

    /// Transform a single MBean (may produce multiple metrics)
    ///
    /// Returns Vec<PrometheusMetric> because:
    /// - CompositeData (e.g., HeapMemoryUsage) → multiple metrics (used, max, init, committed)
    /// - Wildcard matches → multiple MBeans each producing metrics
    fn transform_mbean(&self, mbean: &MBeanData) -> Result<Vec<PrometheusMetric>, TransformError> {
        // Find first matching rule
        for rule in self.rules.iter() {
            if let Some(captures) = rule.matches(mbean)? {
                return self.apply_rule(rule, mbean, &captures);
            }
        }

        // No matching rule - skip this MBean
        Ok(vec![])
    }

    /// Apply a rule to generate metric(s)
    ///
    /// Returns Vec because Composite/Wildcard values produce multiple metrics
    fn apply_rule(
        &self,
        rule: &Rule,
        mbean: &MBeanData,
        captures: &regex::Captures,
    ) -> Result<Vec<PrometheusMetric>, TransformError> {
        let base_name = if rule.name.is_empty() {
            // Default name generation (jmx_exporter behavior):
            // domain_type_attribute format
            self.generate_default_name(mbean)
        } else {
            self.expand_template(&rule.name, captures)
        };

        let base_labels = self.expand_labels(&rule.labels, captures);
        let help = rule.help.as_ref()
            .map(|h| self.expand_template(h, captures));

        // Validate metric name
        let validated_name = self.validate_metric_name(&base_name)?;
        let final_name = if self.lowercase_names {
            validated_name.to_lowercase()
        } else {
            validated_name
        };

        // Validate label names
        let validated_labels = self.validate_labels(&base_labels)?;

        Ok(vec![PrometheusMetric {
            name: final_name,
            metric_type: rule.metric_type,
            help,
            labels: validated_labels,
            value: mbean.value,
            timestamp: None,
        }])
    }

    /// Generate default metric name when rule.name is empty
    /// Follows jmx_exporter convention: domain_propertyKey_attribute
    fn generate_default_name(&self, mbean: &MBeanData) -> String {
        // Extract domain and create snake_case name
        // e.g., "java.lang<type=Memory><HeapMemoryUsage>used"
        //       → "java_lang_memory_heapmemoryusage_used"
        mbean.flattened
            .replace('.', "_")
            .replace('<', "_")
            .replace('>', "_")
            .replace('=', "_")
            .to_lowercase()
    }

    /// Validate Prometheus metric name
    /// Must match: [a-zA-Z_:][a-zA-Z0-9_:]*
    fn validate_metric_name(&self, name: &str) -> Result<String, TransformError> {
        static METRIC_NAME_RE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
        let re = METRIC_NAME_RE.get_or_init(|| {
            regex::Regex::new(r"^[a-zA-Z_:][a-zA-Z0-9_:]*$").unwrap()
        });

        if re.is_match(name) {
            Ok(name.to_string())
        } else {
            // Sanitize: replace invalid chars with underscore
            let sanitized: String = name.chars()
                .enumerate()
                .map(|(i, c)| {
                    if i == 0 {
                        if c.is_ascii_alphabetic() || c == '_' || c == ':' { c } else { '_' }
                    } else {
                        if c.is_ascii_alphanumeric() || c == '_' || c == ':' { c } else { '_' }
                    }
                })
                .collect();
            tracing::warn!(
                original = %name,
                sanitized = %sanitized,
                "Metric name sanitized to match Prometheus naming rules"
            );
            Ok(sanitized)
        }
    }

    /// Validate label names
    /// Must match: [a-zA-Z_][a-zA-Z0-9_]*
    fn validate_labels(&self, labels: &HashMap<String, String>) -> Result<HashMap<String, String>, TransformError> {
        static LABEL_NAME_RE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
        let re = LABEL_NAME_RE.get_or_init(|| {
            regex::Regex::new(r"^[a-zA-Z_][a-zA-Z0-9_]*$").unwrap()
        });

        let mut validated = HashMap::new();
        for (k, v) in labels {
            let key = if re.is_match(k) {
                k.clone()
            } else {
                let sanitized: String = k.chars()
                    .enumerate()
                    .map(|(i, c)| {
                        if i == 0 {
                            if c.is_ascii_alphabetic() || c == '_' { c } else { '_' }
                        } else {
                            if c.is_ascii_alphanumeric() || c == '_' { c } else { '_' }
                        }
                    })
                    .collect();
                tracing::warn!(
                    original = %k,
                    sanitized = %sanitized,
                    "Label name sanitized"
                );
                sanitized
            };
            validated.insert(key, v.clone());
        }
        Ok(validated)
    }

    /// Expand $1, $2, etc. in a template string
    fn expand_template(&self, template: &str, captures: &regex::Captures) -> String {
        let mut result = template.to_string();

        for i in 1..captures.len() {
            if let Some(m) = captures.get(i) {
                let placeholder = format!("${}", i);
                result = result.replace(&placeholder, m.as_str());
            }
        }

        result
    }

    /// Expand labels with capture group values
    fn expand_labels(
        &self,
        labels: &HashMap<String, String>,
        captures: &regex::Captures,
    ) -> HashMap<String, String> {
        labels.iter()
            .map(|(k, v)| {
                let key = if self.lowercase_labels { k.to_lowercase() } else { k.clone() };
                let value = self.expand_template(v, captures);
                (key, value)
            })
            .collect()
    }
}

/// A single Prometheus metric ready for output
#[derive(Debug, Clone)]
pub struct PrometheusMetric {
    pub name: String,
    pub metric_type: MetricType,
    pub help: Option<String>,
    pub labels: HashMap<String, String>,
    pub value: f64,
    pub timestamp: Option<i64>,
}
```

### 4.2 transformer/rules.rs

규칙 파싱 및 매칭 로직을 구현한다.

```rust
//! Rule parsing and matching
//!
//! Implements jmx_exporter-compatible rule syntax with regex pattern matching.

use std::collections::HashMap;
use std::sync::OnceLock;

use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::error::RuleError;

/// Prometheus metric type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MetricType {
    Gauge,
    Counter,
    Untyped,
    #[serde(rename = "GAUGE")]
    GaugeUpper,
    #[serde(rename = "COUNTER")]
    CounterUpper,
    #[serde(rename = "UNTYPED")]
    UntypedUpper,
}

impl MetricType {
    /// Get the lowercase type name for Prometheus output
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Gauge | Self::GaugeUpper => "gauge",
            Self::Counter | Self::CounterUpper => "counter",
            Self::Untyped | Self::UntypedUpper => "untyped",
        }
    }
}

impl Default for MetricType {
    fn default() -> Self {
        Self::Untyped
    }
}

/// A single transformation rule
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Rule {
    /// Regex pattern to match against MBean ObjectName + attribute
    pub pattern: String,

    /// Output metric name (supports $1, $2 capture groups)
    #[serde(default)]
    pub name: String,

    /// Metric type
    #[serde(rename = "type", default)]
    pub metric_type: MetricType,

    /// Static and dynamic labels
    #[serde(default)]
    pub labels: HashMap<String, String>,

    /// Help text for the metric
    #[serde(default)]
    pub help: Option<String>,

    /// Value expression (default: attribute value)
    #[serde(default)]
    pub value: Option<String>,

    /// Value multiplication factor
    #[serde(rename = "valueFactor", default)]
    pub value_factor: Option<f64>,

    // Internal: compiled regex (not serialized)
    #[serde(skip)]
    compiled_pattern: OnceLock<Regex>,
}

impl Rule {
    /// Create a new rule
    pub fn new(pattern: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            pattern: pattern.into(),
            name: name.into(),
            metric_type: MetricType::default(),
            labels: HashMap::new(),
            help: None,
            value: None,
            value_factor: None,
            compiled_pattern: OnceLock::new(),
        }
    }

    /// Compile and cache the regex pattern
    pub fn compile(&self) -> Result<&Regex, RuleError> {
        self.compiled_pattern.get_or_try_init(|| {
            let converted = convert_java_regex(&self.pattern)?;
            Regex::new(&converted).map_err(|e| RuleError::InvalidPattern {
                pattern: self.pattern.clone(),
                source: e,
            })
        })
    }

    /// Check if this rule matches the given MBean string
    pub fn matches(&self, mbean_str: &str) -> Result<Option<regex::Captures<'_>>, RuleError> {
        let regex = self.compile()?;
        Ok(regex.captures(mbean_str))
    }

    /// Apply value factor if configured
    pub fn apply_value_factor(&self, value: f64) -> f64 {
        match self.value_factor {
            Some(factor) => value * factor,
            None => value,
        }
    }
}

/// A collection of rules
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RuleSet {
    #[serde(default)]
    rules: Vec<Rule>,
}

impl RuleSet {
    /// Create a new empty RuleSet
    pub fn new() -> Self {
        Self { rules: Vec::new() }
    }

    /// Add a rule to the set
    pub fn add(&mut self, rule: Rule) {
        self.rules.push(rule);
    }

    /// Get an iterator over the rules
    pub fn iter(&self) -> impl Iterator<Item = &Rule> {
        self.rules.iter()
    }

    /// Compile all rules (validates patterns at startup)
    pub fn compile_all(&self) -> Result<(), RuleError> {
        for (i, rule) in self.rules.iter().enumerate() {
            rule.compile().map_err(|e| RuleError::RuleCompileFailed {
                index: i,
                source: Box::new(e),
            })?;
        }
        Ok(())
    }

    /// Get the number of rules
    pub fn len(&self) -> usize {
        self.rules.len()
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.rules.is_empty()
    }
}

/// Convert Java regex syntax to Rust regex syntax
fn convert_java_regex(pattern: &str) -> Result<String, RuleError> {
    let mut result = pattern.to_string();

    // Named groups: (?<name>...) → (?P<name>...)
    static NAMED_GROUP_RE: OnceLock<Regex> = OnceLock::new();
    let named_group = NAMED_GROUP_RE.get_or_init(|| {
        Regex::new(r"\(\?<([^>]+)>").expect("invalid named group regex")
    });
    result = named_group.replace_all(&result, "(?P<$1>").to_string();

    // Possessive quantifiers: ++, *+, ?+ → +, *, ?
    static POSSESSIVE_RE: OnceLock<Regex> = OnceLock::new();
    let possessive = POSSESSIVE_RE.get_or_init(|| {
        Regex::new(r"([+*?])\+").expect("invalid possessive regex")
    });
    if possessive.is_match(&result) {
        tracing::warn!(
            pattern = %pattern,
            "Possessive quantifiers not supported in Rust regex, converting to greedy"
        );
        result = possessive.replace_all(&result, "$1").to_string();
    }

    // Atomic groups: (?>...) - not supported
    if result.contains("(?>") {
        return Err(RuleError::UnsupportedSyntax {
            pattern: pattern.to_string(),
            feature: "atomic groups (?>...)".to_string(),
        });
    }

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rule_matching() {
        let rule = Rule::new(
            r"java\.lang<type=Memory><(\w+)MemoryUsage>(\w+)",
            "jvm_memory_$1_$2_bytes"
        );

        let mbean = "java.lang<type=Memory><HeapMemoryUsage>used";
        let captures = rule.matches(mbean).unwrap().unwrap();

        assert_eq!(captures.get(1).unwrap().as_str(), "Heap");
        assert_eq!(captures.get(2).unwrap().as_str(), "used");
    }

    #[test]
    fn test_metric_type_serde() {
        let yaml = "gauge";
        let mt: MetricType = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(mt, MetricType::Gauge);

        let yaml = "COUNTER";
        let mt: MetricType = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(mt, MetricType::CounterUpper);
    }

    #[test]
    fn test_java_regex_conversion() {
        // Named groups
        let pattern = r"(?<name>\w+)";
        let converted = convert_java_regex(pattern).unwrap();
        assert_eq!(converted, r"(?P<name>\w+)");

        // Possessive quantifiers
        let pattern = r"\w++";
        let converted = convert_java_regex(pattern).unwrap();
        assert_eq!(converted, r"\w+");
    }

    #[test]
    fn test_atomic_group_error() {
        let pattern = r"(?>foo)bar";
        let result = convert_java_regex(pattern);
        assert!(result.is_err());
    }
}
```

### 4.3 config.rs 업데이트

기존 Config 구조체에 규칙 설정을 통합한다.

```rust
// config.rs 추가 사항

use crate::transformer::{Rule, RuleSet};

/// 전체 설정 구조체
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Jolokia 연결 설정
    pub jolokia: JolokiaConfig,

    /// HTTP 서버 설정
    #[serde(default)]
    pub server: ServerConfig,

    /// 변환 규칙
    #[serde(default)]
    pub rules: Vec<Rule>,

    /// 메트릭명 소문자 변환
    #[serde(rename = "lowercaseOutputName", default)]
    pub lowercase_output_name: bool,

    /// 라벨명 소문자 변환
    #[serde(rename = "lowercaseOutputLabelNames", default)]
    pub lowercase_output_label_names: bool,

    /// MBean 화이트리스트 (glob 패턴)
    #[serde(rename = "whitelistObjectNames", default)]
    pub whitelist_object_names: Vec<String>,

    /// MBean 블랙리스트 (glob 패턴)
    #[serde(rename = "blacklistObjectNames", default)]
    pub blacklist_object_names: Vec<String>,
}

impl Config {
    /// Parse config from YAML file
    pub fn from_file(path: impl AsRef<Path>) -> Result<Self, ConfigError> {
        let content = std::fs::read_to_string(path.as_ref())
            .map_err(|e| ConfigError::ReadFailed {
                path: path.as_ref().to_path_buf(),
                source: e,
            })?;

        Self::from_yaml(&content)
    }

    /// Parse config from YAML string
    pub fn from_yaml(yaml: &str) -> Result<Self, ConfigError> {
        let config: Config = serde_yaml::from_str(yaml)
            .map_err(ConfigError::ParseFailed)?;

        config.validate()?;
        Ok(config)
    }

    /// Validate the configuration
    pub fn validate(&self) -> Result<(), ConfigError> {
        // Validate rules
        let ruleset = self.to_ruleset();
        ruleset.compile_all()
            .map_err(|e| ConfigError::InvalidRule { source: e })?;

        // Validate URL
        if self.jolokia.url.is_empty() {
            return Err(ConfigError::MissingField("jolokia.url".to_string()));
        }

        Ok(())
    }

    /// Convert rules to RuleSet
    pub fn to_ruleset(&self) -> RuleSet {
        let mut ruleset = RuleSet::new();
        for rule in &self.rules {
            ruleset.add(rule.clone());
        }
        ruleset
    }
}
```

---

## 5. 설정 파일 구조 (Configuration Structure)

### 5.1 YAML 스키마 설계

```yaml
# rJMX-Exporter Configuration Schema
# jmx_exporter 호환 + rJMX-Exporter 확장

# === rJMX-Exporter 전용 설정 ===
jolokia:
  url: "http://localhost:8778/jolokia"   # 필수
  username: "admin"                       # 선택
  password: "secret"                      # 선택
  timeout_ms: 5000                        # 선택 (기본: 5000)

server:
  port: 9090                              # 선택 (기본: 9090)
  path: "/metrics"                        # 선택 (기본: /metrics)
  read_timeout_ms: 30000                  # 선택 (기본: 30000)

# === jmx_exporter 호환 설정 ===
lowercaseOutputName: true                 # 선택 (기본: false)
lowercaseOutputLabelNames: true           # 선택 (기본: false)

whitelistObjectNames:                     # 선택
  - "java.lang:*"
  - "java.nio:*"
  - "com.example:*"

blacklistObjectNames:                     # 선택
  - "java.lang:type=MemoryPool,*"

rules:                                    # 필수 (최소 1개 권장)
  - pattern: "java.lang<type=Memory><(\\w+)MemoryUsage>(\\w+)"
    name: "jvm_memory_$1_$2_bytes"
    type: gauge
    help: "JVM $1 memory $2 in bytes"
    labels:
      area: "$1"

  - pattern: "java.lang<type=GarbageCollector,name=(.*)><(\\w+)>"
    name: "jvm_gc_$2"
    type: counter
    help: "JVM garbage collector $2"
    labels:
      gc: "$1"
    valueFactor: 0.001  # ms → seconds
```

### 5.2 Rule 구조체 정의

```rust
/// Transformation Rule - jmx_exporter 호환
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]  // 미지원 필드 경고
pub struct Rule {
    /// MBean 매칭 패턴 (Java regex 호환)
    /// 예: "java.lang<type=Memory><(\\w+)MemoryUsage>(\\w+)"
    pub pattern: String,

    /// 출력 메트릭명 ($1, $2 치환 지원)
    /// 예: "jvm_memory_$1_$2_bytes"
    #[serde(default)]
    pub name: String,

    /// 메트릭 타입
    #[serde(rename = "type", default)]
    pub metric_type: MetricType,

    /// 라벨 맵 (키-값, $1 치환 지원)
    #[serde(default)]
    pub labels: HashMap<String, String>,

    /// HELP 텍스트
    #[serde(default)]
    pub help: Option<String>,

    /// 값 추출 표현식 (기본: 속성값 그대로)
    #[serde(default)]
    pub value: Option<String>,

    /// 값 곱셈 인자 (예: 0.001 for ms→s)
    #[serde(rename = "valueFactor", default)]
    pub value_factor: Option<f64>,
}

/// 메트릭 타입 열거형
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MetricType {
    Gauge,
    Counter,
    #[default]
    Untyped,
    Summary,    // jmx_exporter 호환
    Histogram,  // jmx_exporter 호환 (미지원, 경고만)
}

// serde 역직렬화: 대소문자 무관
impl<'de> Deserialize<'de> for MetricType {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        match s.to_lowercase().as_str() {
            "gauge" => Ok(Self::Gauge),
            "counter" => Ok(Self::Counter),
            "untyped" => Ok(Self::Untyped),
            "summary" => Ok(Self::Summary),
            "histogram" => {
                tracing::warn!("histogram type not fully supported, treating as untyped");
                Ok(Self::Untyped)
            }
            _ => Err(serde::de::Error::unknown_variant(
                &s,
                &["gauge", "counter", "untyped", "summary"],
            )),
        }
    }
}
```

---

## 6. 변환 엔진 설계 (Transform Engine Design)

### 6.1 MBean to Prometheus Metric 변환

#### 변환 파이프라인

```
┌─────────────────┐    ┌─────────────────┐    ┌─────────────────┐
│   MBean Data    │ →  │  Rule Matching  │ →  │  Metric Output  │
│                 │    │                 │    │                 │
│ - objectName    │    │ - Pattern match │    │ - name          │
│ - attribute     │    │ - Capture groups│    │ - type          │
│ - value         │    │ - First match   │    │ - labels        │
│ - type          │    │   wins          │    │ - value         │
└─────────────────┘    └─────────────────┘    └─────────────────┘
```

#### MBean 문자열 포맷

jmx_exporter는 MBean을 특수 포맷으로 평탄화한다:

```
# ObjectName 포맷
java.lang:type=Memory

# jmx_exporter 평탄화 포맷
java.lang<type=Memory><HeapMemoryUsage>used
        ↑             ↑                 ↑
     domain      key properties    attribute path

# CompositeData인 경우
java.lang<type=Memory><HeapMemoryUsage><used>
                                       ↑
                                  composite key
```

#### 변환 로직

```rust
/// MBean 데이터를 평탄화된 문자열로 변환
fn flatten_mbean(mbean: &MBeanData) -> String {
    let mut result = mbean.domain.clone();

    // Key properties를 < > 로 감싸서 추가
    for (key, value) in &mbean.properties {
        result.push_str(&format!("<{}={}>", key, value));
    }

    // Attribute path 추가
    for attr in &mbean.attribute_path {
        result.push_str(&format!("<{}>", attr));
    }

    result
}

/// 첫 번째 매칭 규칙 찾기
fn find_matching_rule<'a>(
    rules: &'a RuleSet,
    mbean_str: &str,
) -> Option<(&'a Rule, regex::Captures<'a>)> {
    for rule in rules.iter() {
        if let Ok(Some(captures)) = rule.matches(mbean_str) {
            return Some((rule, captures));
        }
    }
    None
}
```

### 6.2 Regex 패턴 컴파일 (OnceCell)

성능을 위해 정규식은 최초 사용 시 한 번만 컴파일하고 캐싱한다.

```rust
use std::sync::OnceLock;
use regex::Regex;

pub struct Rule {
    pub pattern: String,
    // ... other fields ...

    /// 컴파일된 정규식 캐시
    #[serde(skip)]
    compiled: OnceLock<Regex>,
}

impl Rule {
    /// 정규식 컴파일 (최초 1회) 또는 캐시 반환
    pub fn compiled_pattern(&self) -> Result<&Regex, RuleError> {
        self.compiled.get_or_try_init(|| {
            let converted = convert_java_regex(&self.pattern)?;
            Regex::new(&converted).map_err(|e| RuleError::InvalidPattern {
                pattern: self.pattern.clone(),
                source: e,
            })
        })
    }
}

// 설정 로드 시 모든 규칙 사전 컴파일 (빠른 실패)
impl RuleSet {
    pub fn compile_all(&self) -> Result<(), RuleError> {
        for (idx, rule) in self.rules.iter().enumerate() {
            rule.compiled_pattern().map_err(|e| {
                RuleError::RuleCompileFailed {
                    index: idx,
                    rule_name: rule.name.clone(),
                    source: Box::new(e),
                }
            })?;
        }
        Ok(())
    }
}
```

### 6.3 Capture Group 치환

`$1`, `$2` 등을 정규식 캡처 결과로 치환한다.

```rust
/// $1, $2, ... 를 캡처 그룹 값으로 치환
///
/// # Replacement Order
/// To avoid $1 replacing the "1" in "$10", we use regex-based replacement
/// that handles all groups atomically.
fn expand_captures(template: &str, captures: &regex::Captures) -> String {
    use std::sync::OnceLock;

    static PLACEHOLDER_RE: OnceLock<regex::Regex> = OnceLock::new();
    let re = PLACEHOLDER_RE.get_or_init(|| {
        // Match $1, $2, ..., $99 (two digits max)
        regex::Regex::new(r"\$(\d{1,2})").unwrap()
    });

    re.replace_all(template, |caps: &regex::Captures| {
        let index: usize = caps[1].parse().unwrap_or(0);
        captures.get(index)
            .map(|m| m.as_str().to_string())
            .unwrap_or_else(|| format!("${}", index))
    }).to_string()
}

/// Named capture 지원: ${name}
fn expand_named_captures(template: &str, captures: &regex::Captures) -> String {
    static NAMED_RE: OnceLock<Regex> = OnceLock::new();
    let re = NAMED_RE.get_or_init(|| {
        Regex::new(r"\$\{([a-zA-Z_][a-zA-Z0-9_]*)\}").unwrap()
    });

    re.replace_all(template, |caps: &regex::Captures| {
        let name = &caps[1];
        captures.name(name)
            .map(|m| m.as_str().to_string())
            .unwrap_or_else(|| format!("${{{}}}", name))
    }).to_string()
}
```

---

## 7. Prometheus 출력 포맷 (Prometheus Output Format)

### 7.1 Exposition Format 사양

Prometheus Text Exposition Format (version 0.0.4):

```
# HELP <metric_name> <help_text>
# TYPE <metric_name> <type>
<metric_name>{<label1>="<value1>",<label2>="<value2>"} <value> [<timestamp>]
```

### 7.2 HELP, TYPE 라인

```rust
/// Prometheus exposition format 생성기
///
/// # Usage
/// Create a new instance for each scrape request to avoid stale state.
/// The `seen_metrics` set is used within a single `format()` call to
/// ensure HELP/TYPE are emitted only once per metric name.
pub struct PrometheusFormatter;

impl PrometheusFormatter {
    pub fn new() -> Self {
        Self
    }

    /// 메트릭을 문자열로 포맷
    ///
    /// # Note
    /// Each call to format() is independent - no state is retained between calls.
    /// HELP/TYPE lines are emitted once per unique metric name within this call.
    pub fn format(&self, metrics: &[PrometheusMetric]) -> String {
        let mut output = String::with_capacity(metrics.len() * 100);

        // Track seen metrics LOCALLY within this format call
        // This ensures each scrape response is complete and independent
        let mut seen_metrics: HashSet<String> = HashSet::new();

        // 같은 이름의 메트릭 그룹화
        let grouped = Self::group_by_name(metrics);

        for (name, group) in grouped {
            // HELP/TYPE는 메트릭당 한 번만
            if !seen_metrics.contains(&name) {
                seen_metrics.insert(name.clone());

                // HELP 라인
                if let Some(help) = &group[0].help {
                    output.push_str(&format!(
                        "# HELP {} {}\n",
                        name,
                        Self::escape_help(help)
                    ));
                }

                // TYPE 라인
                output.push_str(&format!(
                    "# TYPE {} {}\n",
                    name,
                    group[0].metric_type.as_str()
                ));
            }

            // 메트릭 값 라인들
            for metric in group {
                output.push_str(&self.format_metric_line(metric));
                output.push('\n');
            }
        }

        output
    }

    fn format_metric_line(&self, metric: &PrometheusMetric) -> String {
        let mut line = metric.name.clone();

        // 라벨 (키 순서 정렬하여 출력 안정성 확보 - 테스트/스냅샷 비교용)
        if !metric.labels.is_empty() {
            let mut sorted_labels: Vec<(&String, &String)> = metric.labels.iter().collect();
            sorted_labels.sort_by_key(|(k, _)| *k);

            let labels: Vec<String> = sorted_labels.iter()
                .map(|(k, v)| format!("{}=\"{}\"", k, Self::escape_label_value(v)))
                .collect();
            line.push('{');
            line.push_str(&labels.join(","));
            line.push('}');
        }

        // 값
        line.push(' ');
        line.push_str(&Self::format_value(metric.value));

        // 타임스탬프 (선택)
        if let Some(ts) = metric.timestamp {
            line.push(' ');
            line.push_str(&ts.to_string());
        }

        line
    }

    /// 값 포맷 (정수면 정수로, 아니면 부동소수점)
    fn format_value(value: f64) -> String {
        if value.is_nan() {
            "NaN".to_string()
        } else if value.is_infinite() {
            if value.is_sign_positive() { "+Inf" } else { "-Inf" }.to_string()
        } else if value.fract() == 0.0 && value.abs() < 1e15 {
            format!("{}", value as i64)
        } else {
            format!("{:e}", value)  // 과학적 표기법
        }
    }
}
```

### 7.3 Label Escaping

Prometheus 라벨 값에서 이스케이프가 필요한 문자:

| 문자 | 이스케이프 |
|------|-----------|
| `\` | `\\` |
| `"` | `\"` |
| `\n` | `\n` |

```rust
/// 라벨 값 이스케이프
fn escape_label_value(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for c in value.chars() {
        match c {
            '\\' => escaped.push_str("\\\\"),
            '"' => escaped.push_str("\\\""),
            '\n' => escaped.push_str("\\n"),
            _ => escaped.push(c),
        }
    }
    escaped
}

/// HELP 텍스트 이스케이프
fn escape_help(help: &str) -> String {
    help.replace('\\', "\\\\").replace('\n', "\\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_label_escaping() {
        assert_eq!(escape_label_value("hello"), "hello");
        assert_eq!(escape_label_value("hello\"world"), "hello\\\"world");
        assert_eq!(escape_label_value("line1\nline2"), "line1\\nline2");
        assert_eq!(escape_label_value("path\\to\\file"), "path\\\\to\\\\file");
    }
}
```

### 7.4 출력 예시

**입력 MBean:**
```json
{
  "objectName": "java.lang:type=Memory",
  "attribute": "HeapMemoryUsage",
  "value": {
    "used": 123456789,
    "max": 536870912
  }
}
```

**규칙:**
```yaml
- pattern: "java.lang<type=Memory><HeapMemoryUsage>(\\w+)"
  name: "jvm_memory_heap_$1_bytes"
  type: gauge
  help: "JVM heap memory $1"
  labels:
    area: "heap"
```

**출력:**
```
# HELP jvm_memory_heap_used_bytes JVM heap memory used
# TYPE jvm_memory_heap_used_bytes gauge
jvm_memory_heap_used_bytes{area="heap"} 123456789
# HELP jvm_memory_heap_max_bytes JVM heap memory max
# TYPE jvm_memory_heap_max_bytes gauge
jvm_memory_heap_max_bytes{area="heap"} 536870912
```

---

## 8. 테스트 계획 (Test Plan)

### 8.1 Rule Parsing 테스트

```rust
#[cfg(test)]
mod rule_parsing_tests {
    use super::*;

    #[test]
    fn test_parse_minimal_rule() {
        let yaml = r#"
            pattern: "java.lang<type=Memory>.*"
            name: "jvm_memory"
        "#;

        let rule: Rule = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(rule.pattern, "java.lang<type=Memory>.*");
        assert_eq!(rule.name, "jvm_memory");
        assert_eq!(rule.metric_type, MetricType::Untyped);
    }

    #[test]
    fn test_parse_full_rule() {
        let yaml = r#"
            pattern: "java.lang<type=Memory><(\\w+)MemoryUsage>(\\w+)"
            name: "jvm_memory_$1_$2_bytes"
            type: gauge
            help: "JVM $1 memory $2"
            labels:
              area: "$1"
              metric: "$2"
            valueFactor: 0.001
        "#;

        let rule: Rule = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(rule.metric_type, MetricType::Gauge);
        assert_eq!(rule.labels.get("area"), Some(&"$1".to_string()));
        assert_eq!(rule.value_factor, Some(0.001));
    }

    #[test]
    fn test_parse_metric_type_case_insensitive() {
        for type_str in &["gauge", "Gauge", "GAUGE"] {
            let yaml = format!("pattern: \".*\"\nname: \"test\"\ntype: {}", type_str);
            let rule: Rule = serde_yaml::from_str(&yaml).unwrap();
            assert!(matches!(rule.metric_type, MetricType::Gauge | MetricType::GaugeUpper));
        }
    }

    #[test]
    fn test_reject_unknown_fields() {
        let yaml = r#"
            pattern: ".*"
            name: "test"
            unknownField: "value"
        "#;

        let result: Result<Rule, _> = serde_yaml::from_str(yaml);
        assert!(result.is_err());
    }
}
```

### 8.2 Transform 테스트

```rust
#[cfg(test)]
mod transform_tests {
    use super::*;

    fn create_test_engine() -> TransformEngine {
        let mut ruleset = RuleSet::new();
        ruleset.add(Rule {
            pattern: r"java\.lang<type=Memory><(\w+)MemoryUsage>(\w+)".to_string(),
            name: "jvm_memory_$1_$2_bytes".to_string(),
            metric_type: MetricType::Gauge,
            labels: [("area".to_string(), "$1".to_string())].into(),
            help: Some("JVM $1 memory $2".to_string()),
            ..Default::default()
        });
        TransformEngine::new(ruleset)
    }

    #[test]
    fn test_basic_transform() {
        let engine = create_test_engine();
        let mbean = MBeanData {
            flattened: "java.lang<type=Memory><HeapMemoryUsage>used".to_string(),
            value: 123456789.0,
        };

        let result = engine.transform(&[mbean]).unwrap();
        assert_eq!(result.len(), 1);

        let metric = &result[0];
        assert_eq!(metric.name, "jvm_memory_Heap_used_bytes");
        assert_eq!(metric.labels.get("area"), Some(&"Heap".to_string()));
        assert_eq!(metric.value, 123456789.0);
    }

    #[test]
    fn test_no_matching_rule() {
        let engine = create_test_engine();
        let mbean = MBeanData {
            flattened: "some.other<type=Unknown>attr".to_string(),
            value: 42.0,
        };

        let result = engine.transform(&[mbean]).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_value_factor() {
        let mut ruleset = RuleSet::new();
        ruleset.add(Rule {
            pattern: r".*".to_string(),
            name: "test_metric".to_string(),
            value_factor: Some(0.001),
            ..Default::default()
        });
        let engine = TransformEngine::new(ruleset);

        let mbean = MBeanData {
            flattened: "test".to_string(),
            value: 1000.0,
        };

        let result = engine.transform(&[mbean]).unwrap();
        assert_eq!(result[0].value, 1.0);  // 1000 * 0.001
    }

    #[test]
    fn test_lowercase_transform() {
        let mut engine = create_test_engine();
        engine.lowercase_names = true;
        engine.lowercase_labels = true;

        let mbean = MBeanData {
            flattened: "java.lang<type=Memory><HeapMemoryUsage>used".to_string(),
            value: 100.0,
        };

        let result = engine.transform(&[mbean]).unwrap();
        let metric = &result[0];

        assert_eq!(metric.name, "jvm_memory_heap_used_bytes");
        assert_eq!(metric.labels.get("area"), Some(&"heap".to_string()));
    }
}
```

### 8.3 Snapshot 테스트 (insta)

```rust
#[cfg(test)]
mod snapshot_tests {
    use super::*;
    use insta::assert_snapshot;

    #[test]
    fn test_prometheus_output_snapshot() {
        let metrics = vec![
            PrometheusMetric {
                name: "jvm_memory_heap_used_bytes".to_string(),
                metric_type: MetricType::Gauge,
                help: Some("JVM heap memory used".to_string()),
                labels: [("area".to_string(), "heap".to_string())].into(),
                value: 123456789.0,
                timestamp: None,
            },
            PrometheusMetric {
                name: "jvm_memory_heap_max_bytes".to_string(),
                metric_type: MetricType::Gauge,
                help: Some("JVM heap memory max".to_string()),
                labels: [("area".to_string(), "heap".to_string())].into(),
                value: 536870912.0,
                timestamp: None,
            },
        ];

        let mut formatter = PrometheusFormatter::new();
        let output = formatter.format(&metrics);

        assert_snapshot!("prometheus_output", output);
    }

    #[test]
    fn test_jmxexporter_compat_snapshot() {
        // jmx_exporter와 동일한 설정 + 입력으로 출력 비교
        let config = r#"
            rules:
              - pattern: "java.lang<type=Memory><(\\w+)MemoryUsage>(\\w+)"
                name: "jvm_memory_$1_$2_bytes"
                type: gauge
                labels:
                  area: "$1"
        "#;

        let ruleset: RuleSet = serde_yaml::from_str(config).unwrap();
        let engine = TransformEngine::new(ruleset);

        let mbeans = vec![
            MBeanData::new("java.lang<type=Memory><HeapMemoryUsage>used", 123456789.0),
            MBeanData::new("java.lang<type=Memory><HeapMemoryUsage>max", 536870912.0),
            MBeanData::new("java.lang<type=Memory><NonHeapMemoryUsage>used", 45678901.0),
        ];

        let metrics = engine.transform(&mbeans).unwrap();
        let mut formatter = PrometheusFormatter::new();
        let output = formatter.format(&metrics);

        assert_snapshot!("jmxexporter_compat", output);
    }
}
```

### 8.4 테스트 커버리지 목표

| 모듈 | 라인 커버리지 | 브랜치 커버리지 |
|------|---------------|-----------------|
| transformer/rules.rs | > 90% | > 80% |
| transformer/mod.rs | > 85% | > 75% |
| config.rs (규칙 부분) | > 90% | > 80% |

---

## 9. 완료 기준 (Definition of Done)

### 9.1 기능 완료 기준

- [ ] **YAML 규칙 파싱**
  - [ ] 모든 P0 옵션 지원 (pattern, name, type, labels, help)
  - [ ] 유효하지 않은 설정에 대한 명확한 에러 메시지
  - [ ] jmx_exporter 설정 파일 호환성 테스트 통과

- [ ] **Regex 변환**
  - [ ] Java regex → Rust regex 자동 변환
  - [ ] 지원되지 않는 문법에 대한 경고/에러
  - [ ] 캡처 그룹 ($1, $2) 정상 동작

- [ ] **메트릭 변환**
  - [ ] MBean 평탄화 (jmx_exporter 포맷)
  - [ ] 규칙 매칭 및 메트릭 생성
  - [ ] lowercaseOutputName/Labels 지원

- [ ] **Prometheus 출력**
  - [ ] HELP/TYPE 라인 출력
  - [ ] 라벨 이스케이프 처리
  - [ ] 값 포맷 (정수/부동소수점/NaN/Inf)

### 9.2 품질 기준

- [ ] **테스트**
  - [ ] 단위 테스트 커버리지 > 80%
  - [ ] insta 스냅샷 테스트 추가
  - [ ] jmx_exporter 호환성 테스트

- [ ] **문서**
  - [ ] 모든 pub 아이템에 rustdoc 주석
  - [ ] 규칙 작성 가이드 (examples/)

- [ ] **코드 품질**
  - [ ] `cargo clippy -- -D warnings` 통과
  - [ ] `cargo fmt -- --check` 통과
  - [ ] 모든 `unwrap()` 제거 또는 명확한 주석

### 9.3 성능 기준

- [ ] **벤치마크**
  - [ ] criterion 벤치마크 작성
  - [ ] 1000 메트릭 변환 < 5ms
  - [ ] 메모리 사용량 프로파일링

### 9.4 체크리스트

```bash
# 완료 검증 스크립트
#!/bin/bash

echo "=== Phase 3 Completion Check ==="

# 빌드
cargo build --release || exit 1
echo "✓ Build passed"

# 테스트
cargo test || exit 1
echo "✓ Tests passed"

# Clippy
cargo clippy -- -D warnings || exit 1
echo "✓ Clippy passed"

# Format
cargo fmt -- --check || exit 1
echo "✓ Format check passed"

# 문서
cargo doc --no-deps || exit 1
echo "✓ Doc generation passed"

echo ""
echo "=== All checks passed! ==="
```

---

## 10. 부록 (Appendix)

### A. jmx_exporter 설정 예시

```yaml
# 실제 jmx_exporter 설정 예시 (Kafka)
lowercaseOutputName: true
lowercaseOutputLabelNames: true

whitelistObjectNames:
  - "kafka.server:*"
  - "kafka.controller:*"
  - "java.lang:*"

rules:
  # Kafka 요청 처리량
  - pattern: "kafka.server<type=BrokerTopicMetrics, name=(MessagesInPerSec|BytesInPerSec|BytesOutPerSec), topic=(.+)><Count>"
    name: "kafka_server_brokertopicmetrics_$1_total"
    type: counter
    labels:
      topic: "$2"

  # Kafka 컨슈머 그룹
  - pattern: "kafka.server<type=group-coordinator-metrics, name=(.+)><Value>"
    name: "kafka_server_group_coordinator_$1"
    type: gauge

  # JVM 메모리
  - pattern: "java.lang<type=Memory><(\\w+)MemoryUsage>(\\w+)"
    name: "jvm_memory_$1_$2_bytes"
    type: gauge
    help: "JVM $1 memory $2"
    labels:
      area: "$1"
```

### B. 에러 코드 정의

| 코드 | 에러 | 설명 |
|------|------|------|
| `E001` | InvalidPattern | 정규식 컴파일 실패 |
| `E002` | UnsupportedSyntax | 지원되지 않는 Java regex 문법 |
| `E003` | InvalidMetricName | Prometheus 메트릭명 규칙 위반 |
| `E004` | InvalidLabelName | Prometheus 라벨명 규칙 위반 |
| `E005` | CaptureGroupMissing | 존재하지 않는 캡처 그룹 참조 |

### C. 참고 자료

- [jmx_exporter Configuration](https://github.com/prometheus/jmx_exporter#configuration)
- [Prometheus Exposition Format](https://prometheus.io/docs/instrumenting/exposition_formats/)
- [Rust regex Syntax](https://docs.rs/regex/latest/regex/#syntax)
- [Jolokia Protocol](https://jolokia.org/reference/html/protocol.html)

---

*문서 버전: 1.0*
*최종 수정: 2026-02-01*
*작성자: rJMX-Exporter Team*
