# Phase 4: 검증 및 프로덕션 준비 (Validation & Production Readiness)

> **rJMX-Exporter 구현 계획서 - Phase 4**

---

## 1. 개요 (Overview)

Phase 4는 rJMX-Exporter의 최종 검증 및 프로덕션 배포 준비 단계입니다. 이 단계에서는 성능 목표 달성 여부를 검증하고, 통합 테스트를 통해 안정성을 확보하며, 실제 운영 환경에 배포할 수 있도록 준비합니다.

### 1.1 Phase 4 범위

| 영역 | 설명 |
|------|------|
| **벤치마크** | Java 버전(jmx_exporter) 대비 성능 측정 및 비교 |
| **통합 테스트** | End-to-end 테스트 및 Docker 기반 테스트 환경 구축 |
| **CLI 완성** | clap 기반 CLI 구현 완료 |
| **문서화** | 사용자 가이드, 마이그레이션 가이드 작성 |
| **릴리스** | CI/CD 파이프라인, 멀티 플랫폼 바이너리 빌드 |

### 1.2 선행 조건

- Phase 1 (Foundation): 프로젝트 구조 및 기본 서버 완료
- Phase 2 (Data Collection): Jolokia 데이터 수집 완료
- Phase 3 (Transform Engine): 규칙 기반 변환 엔진 완료

---

## 2. 목표 (Goals)

### 2.1 성능 목표

| 지표 | 목표값 | jmx_exporter (비교) | 비고 |
|------|--------|---------------------|------|
| **메모리 사용량** | < 10MB | ~50MB+ (standalone) | 80% 이상 절감 |
| **시작 시간** | < 100ms | 수 초 | JVM 시작 시간 제거 |
| **스크레이프 지연** | < 10ms | 수십 ms | 수집부터 응답까지 |
| **바이너리 크기** | < 10MB | JVM 필요 (~200MB+) | 네이티브 바이너리 |
| **대상 앱 영향** | 0% | 공유 힙/GC (agent mode) | 완전 격리 |

### 2.2 품질 목표

| 항목 | 목표 |
|------|------|
| 테스트 커버리지 | 80% 이상 |
| 통합 테스트 | 주요 시나리오 100% 커버 |
| 문서화 | 모든 public API 문서화 |
| CI/CD | 모든 PR에 대한 자동 테스트 |

---

## 3. 벤치마크 계획 (Benchmark Plan)

### 3.1 메모리 사용량 측정

#### 측정 방법

```bash
# 1. rJMX-Exporter 메모리 측정 (Linux)
/usr/bin/time -v ./rjmx-exporter -c config.yaml 2>&1 | grep "Maximum resident"

# 2. macOS에서 측정
/usr/bin/time -l ./rjmx-exporter -c config.yaml

# 3. 프로파일링 도구 사용
heaptrack ./rjmx-exporter -c config.yaml
```

#### 벤치마크 코드

```rust
// benches/memory.rs
use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};

struct TrackingAllocator;

static ALLOCATED: AtomicUsize = AtomicUsize::new(0);

unsafe impl GlobalAlloc for TrackingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        ALLOCATED.fetch_add(layout.size(), Ordering::SeqCst);
        System.alloc(layout)
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        ALLOCATED.fetch_sub(layout.size(), Ordering::SeqCst);
        System.dealloc(ptr, layout)
    }
}

#[global_allocator]
static GLOBAL: TrackingAllocator = TrackingAllocator;

pub fn current_memory_usage() -> usize {
    ALLOCATED.load(Ordering::SeqCst)
}
```

#### 측정 시나리오

| 시나리오 | 설명 | 목표 |
|----------|------|------|
| Idle | 서버 시작 후 대기 상태 | < 5MB |
| Single Scrape | 단일 스크레이프 요청 처리 | < 8MB |
| Concurrent Scrapes | 10개 동시 스크레이프 | < 10MB |
| Large Response | 1000+ 메트릭 처리 | < 15MB |

### 3.2 시작 시간 측정

#### 측정 방법

```bash
# 1. hyperfine 사용 (권장)
hyperfine --warmup 3 './rjmx-exporter -c config.yaml --dry-run'

# 2. 내장 측정
./rjmx-exporter -c config.yaml --startup-time

# 3. 수동 측정
time ./rjmx-exporter -c config.yaml --dry-run
```

#### 벤치마크 코드

```rust
// src/main.rs
use std::time::Instant;

fn main() -> anyhow::Result<()> {
    let start = Instant::now();

    // 초기화 코드...
    let config = Config::load(&args.config)?;
    let rules = compile_rules(&config.rules)?;

    let startup_duration = start.elapsed();
    tracing::info!(
        startup_ms = startup_duration.as_millis(),
        "Startup completed"
    );

    // 서버 시작...
}
```

#### 측정 항목

| 단계 | 목표 시간 | 설명 |
|------|-----------|------|
| Config 로드 | < 10ms | YAML 파싱 |
| Regex 컴파일 | < 50ms | 모든 규칙 사전 컴파일 |
| 서버 바인딩 | < 10ms | TCP 리스너 설정 |
| **전체** | **< 100ms** | 요청 수신 가능 상태까지 |

### 3.3 스크레이프 지연 측정

#### 측정 방법

```bash
# 1. curl을 이용한 측정
curl -w "@curl-format.txt" -o /dev/null -s http://localhost:9090/metrics

# curl-format.txt:
#     time_total:  %{time_total}s\n
#     time_connect:  %{time_connect}s\n
#     time_starttransfer:  %{time_starttransfer}s\n

# 2. wrk를 이용한 부하 테스트
wrk -t4 -c10 -d30s http://localhost:9090/metrics
```

#### 벤치마크 코드

```rust
// benches/scrape_latency.rs
use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn bench_scrape_latency(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let client = reqwest::Client::new();

    c.bench_function("scrape_latency", |b| {
        b.to_async(&rt).iter(|| async {
            let response = client
                .get("http://localhost:9090/metrics")
                .send()
                .await
                .unwrap();
            black_box(response.text().await.unwrap())
        })
    });
}

criterion_group!(benches, bench_scrape_latency);
criterion_main!(benches);
```

#### 지연 분석 항목

| 구간 | 목표 | 설명 |
|------|------|------|
| Jolokia 요청 | < 5ms | HTTP 클라이언트 요청 (캐시 제외) |
| JSON 파싱 | < 2ms | serde_json 역직렬화 |
| 변환 처리 | < 2ms | 규칙 적용 및 포맷 변환 |
| 응답 전송 | < 1ms | Prometheus 포맷 응답 |
| **전체** | **< 10ms** | 요청 수신부터 응답 완료까지 |

### 3.4 jmx_exporter 비교 테스트

#### 테스트 환경

**환경 일관성 요구사항 (재현성 확보):**
- 동일 Docker 컨테이너 리소스 제한 (CPU: 1 core, Memory: 512MB)
- 동일 config.yaml 사용 (동일 규칙 개수, 동일 MBean 대상)
- 웜업 단계 포함 (첫 10회 요청은 측정에서 제외)
- 결과 저장 포맷: JSON (자동화 파싱용)

```yaml
# docker-compose.benchmark.yaml
version: '3.8'
services:
  java-app:
    image: openjdk:17-slim
    command: |
      java -javaagent:/jolokia-agent.jar=port=8778,host=0.0.0.0
           -jar /app.jar
    ports:
      - "8778:8778"
    deploy:
      resources:
        limits:
          cpus: '1'
          memory: 512M

  jmx-exporter:
    image: bitnami/jmx-exporter:latest
    ports:
      - "9091:9091"
    environment:
      - JMX_EXPORTER_CONFIG=/config.yaml
    deploy:
      resources:
        limits:
          cpus: '1'
          memory: 512M

  rjmx-exporter:
    build: .
    ports:
      - "9090:9090"
    command: ["./rjmx-exporter", "-c", "/config.yaml"]
    deploy:
      resources:
        limits:
          cpus: '1'
          memory: 512M
```

#### 비교 항목

| 항목 | jmx_exporter | rJMX-Exporter | 측정 방법 |
|------|--------------|---------------|-----------|
| 메모리 (RSS) | 측정 | 측정 | `docker stats` |
| 시작 시간 | 측정 | 측정 | 로그 타임스탬프 |
| P50 지연 | 측정 | 측정 | wrk 결과 |
| P99 지연 | 측정 | 측정 | wrk 결과 |
| 초당 처리량 | 측정 | 측정 | wrk 결과 |

#### 벤치마크 스크립트

```bash
#!/bin/bash
# scripts/benchmark.sh

set -e

RESULTS_FILE="benchmark-results-$(date +%Y%m%d-%H%M%S).json"
WARMUP_REQUESTS=10
TEST_DURATION=30

echo "=== rJMX-Exporter Benchmark ==="
echo "Output: $RESULTS_FILE"

# Warmup phase
echo "Warming up..."
for i in $(seq 1 $WARMUP_REQUESTS); do
    curl -s http://localhost:9090/metrics > /dev/null
    curl -s http://localhost:9091/metrics > /dev/null
done

# 메모리 측정
echo "Memory Usage:"
RJMX_MEM=$(docker stats --no-stream --format "{{.MemUsage}}" rjmx-exporter | cut -d'/' -f1 | tr -d ' ')
JMX_MEM=$(docker stats --no-stream --format "{{.MemUsage}}" jmx-exporter | cut -d'/' -f1 | tr -d ' ')
echo "  rJMX-Exporter: $RJMX_MEM"
echo "  jmx_exporter: $JMX_MEM"

# 지연 측정 (JSON 출력 포맷)
echo -e "\nLatency Test (rJMX-Exporter):"
RJMX_LATENCY=$(wrk -t4 -c10 -d${TEST_DURATION}s --latency http://localhost:9090/metrics 2>&1)
echo "$RJMX_LATENCY"

echo -e "\nLatency Test (jmx_exporter):"
JMX_LATENCY=$(wrk -t4 -c10 -d${TEST_DURATION}s --latency http://localhost:9091/metrics 2>&1)
echo "$JMX_LATENCY"

# 결과를 JSON으로 저장
cat > "$RESULTS_FILE" << EOF
{
  "timestamp": "$(date -Iseconds)",
  "environment": {
    "cpu_limit": "1 core",
    "memory_limit": "512MB",
    "warmup_requests": $WARMUP_REQUESTS,
    "test_duration_seconds": $TEST_DURATION
  },
  "results": {
    "rjmx_exporter": {
      "memory": "$RJMX_MEM"
    },
    "jmx_exporter": {
      "memory": "$JMX_MEM"
    }
  },
  "raw_output": {
    "rjmx_exporter_wrk": $(echo "$RJMX_LATENCY" | jq -Rs .),
    "jmx_exporter_wrk": $(echo "$JMX_LATENCY" | jq -Rs .)
  }
}
EOF

echo -e "\nResults saved to $RESULTS_FILE"
```

---

## 4. 성능 최적화 (Performance Optimization)

### 4.1 Zero-Copy 파싱

#### 현재 문제

```rust
// 비효율적: 모든 문자열 복사
#[derive(Deserialize)]
struct JolokiaResponse {
    value: HashMap<String, String>,  // String 복사 발생
}
```

#### 최적화 방안

```rust
// 효율적: 빌려온 참조 사용
#[derive(Deserialize)]
struct JolokiaResponse<'a> {
    #[serde(borrow)]
    value: HashMap<&'a str, &'a str>,  // Zero-copy
}

// 또는 Cow 사용
use std::borrow::Cow;

#[derive(Deserialize)]
struct JolokiaResponse<'a> {
    #[serde(borrow)]
    value: HashMap<Cow<'a, str>, Cow<'a, str>>,
}
```

#### 구현 체크리스트

- [ ] `JolokiaResponse`에 lifetime 매개변수 추가
- [ ] `#[serde(borrow)]` 속성 적용
- [ ] 응답 버퍼 lifetime 관리 구현
- [ ] 벤치마크로 개선 효과 측정

### 4.2 Pre-compiled Regex

#### 현재 문제

```rust
// 비효율적: 매 요청마다 컴파일
fn apply_rule(pattern: &str, input: &str) -> bool {
    let re = Regex::new(pattern).unwrap();  // 매번 컴파일
    re.is_match(input)
}
```

#### 최적화 방안

```rust
use once_cell::sync::Lazy;
use regex::Regex;
use std::collections::HashMap;
use std::sync::RwLock;

// 방법 1: OnceCell + HashMap 캐시
static REGEX_CACHE: Lazy<RwLock<HashMap<String, Regex>>> =
    Lazy::new(|| RwLock::new(HashMap::new()));

fn get_or_compile_regex(pattern: &str) -> Regex {
    // 읽기 잠금으로 캐시 확인
    if let Some(re) = REGEX_CACHE.read().unwrap().get(pattern) {
        return re.clone();
    }

    // 쓰기 잠금으로 컴파일 및 저장
    let mut cache = REGEX_CACHE.write().unwrap();
    let re = Regex::new(pattern).expect("Invalid regex");
    cache.insert(pattern.to_string(), re.clone());
    re
}

// 방법 2: 규칙 구조체에 컴파일된 regex 저장 (권장)
pub struct CompiledRule {
    pub pattern: Regex,  // 사전 컴파일
    pub name_template: String,
    pub metric_type: MetricType,
    pub labels: HashMap<String, String>,
}

impl CompiledRule {
    pub fn compile(rule: &Rule) -> Result<Self, regex::Error> {
        Ok(Self {
            pattern: Regex::new(&rule.pattern)?,
            name_template: rule.name.clone(),
            metric_type: rule.metric_type,
            labels: rule.labels.clone(),
        })
    }
}
```

#### 구현 체크리스트

- [ ] `CompiledRule` 구조체 정의
- [ ] 시작 시점에 모든 규칙 사전 컴파일
- [ ] 컴파일 실패 시 적절한 에러 메시지
- [ ] 벤치마크로 개선 효과 측정

### 4.3 Connection Pooling

#### Reqwest 클라이언트 최적화

```rust
use reqwest::Client;
use std::time::Duration;

pub struct JolokiaClient {
    client: Client,
    base_url: String,
}

impl JolokiaClient {
    pub fn new(config: &JolokiaConfig) -> Result<Self, reqwest::Error> {
        let client = Client::builder()
            // 연결 풀 설정
            .pool_max_idle_per_host(10)
            .pool_idle_timeout(Duration::from_secs(30))

            // 타임아웃 설정
            .connect_timeout(Duration::from_secs(5))
            .timeout(Duration::from_secs(10))

            // Keep-Alive 활성화
            .tcp_keepalive(Duration::from_secs(60))

            // HTTP/2 활성화 (Jolokia 서버 지원 시)
            .http2_prior_knowledge()

            // TLS 설정
            .use_rustls_tls()

            .build()?;

        Ok(Self {
            client,
            base_url: config.url.clone(),
        })
    }
}
```

#### 구현 체크리스트

- [ ] 싱글톤 `Client` 인스턴스 유지
- [ ] 적절한 풀 크기 설정
- [ ] Keep-Alive 타임아웃 조정
- [ ] 연결 재사용 메트릭 추가

### 4.4 바이너리 크기 최적화

#### Cargo.toml 설정

```toml
[profile.release]
# LTO (Link Time Optimization)
lto = true

# 단일 코드 생성 유닛 (느린 빌드, 작은 바이너리)
codegen-units = 1

# 최적화 레벨
opt-level = "z"  # 크기 최적화 ("s"도 가능)

# 패닉 시 abort (unwinding 제거)
panic = "abort"

# 디버그 정보 제거
strip = true

# 심볼 테이블 제거
debug = false
```

#### 추가 최적화

```bash
# UPX 압축 (선택적)
upx --best --lzma target/release/rjmx-exporter

# 크기 분석
cargo bloat --release --crates
cargo bloat --release -n 20
```

#### 의존성 크기 분석

```bash
# cargo-bloat 설치
cargo install cargo-bloat

# 크기 기여도 분석
cargo bloat --release --crates

# 함수별 크기 분석
cargo bloat --release -n 30
```

#### 예상 크기

| 설정 | 예상 크기 |
|------|-----------|
| 기본 release | ~15MB |
| LTO + strip | ~8MB |
| opt-level = "z" | ~6MB |
| UPX 압축 | ~3MB |

---

## 5. 통합 테스트 (Integration Tests)

### 5.1 End-to-End 테스트 시나리오

#### 테스트 매트릭스

| 시나리오 | 설명 | 우선순위 |
|----------|------|----------|
| 기본 스크레이프 | 단일 타겟에서 메트릭 수집 | P0 |
| 멀티 타겟 | 여러 Jolokia 엔드포인트 수집 | P1 |
| 규칙 변환 | pattern → name 변환 검증 | P0 |
| 레이블 처리 | 정적/동적 레이블 검증 | P0 |
| 에러 처리 | 타겟 다운 시 graceful 처리 | P1 |
| 설정 검증 | 잘못된 설정 거부 | P0 |
| 동시 요청 | 100 동시 스크레이프 처리 | P1 |

#### 테스트 코드 예시

```rust
// tests/integration/scrape_test.rs
use assert_cmd::Command;
use predicates::prelude::*;
use std::time::Duration;
use tokio::time::sleep;

#[tokio::test]
async fn test_basic_scrape() {
    // 1. 테스트 Jolokia 서버 시작
    let mock_server = start_mock_jolokia_server().await;

    // 2. rjmx-exporter 시작
    let config = create_test_config(&mock_server.url());
    let mut cmd = Command::cargo_bin("rjmx-exporter").unwrap();
    let mut child = cmd
        .arg("-c")
        .arg(&config.path())
        .spawn()
        .unwrap();

    // 3. 서버 준비 대기
    sleep(Duration::from_millis(100)).await;

    // 4. /metrics 엔드포인트 호출
    let response = reqwest::get("http://localhost:9090/metrics")
        .await
        .unwrap();

    // 5. 검증
    assert_eq!(response.status(), 200);
    let body = response.text().await.unwrap();
    assert!(body.contains("jvm_memory_heap_used_bytes"));

    // 6. 정리
    child.kill().unwrap();
}

#[tokio::test]
async fn test_rule_transformation() {
    // 규칙 변환 테스트
    let config = r#"
        jolokia:
          url: "http://localhost:8778/jolokia"
        rules:
          - pattern: "java.lang<type=Memory><HeapMemoryUsage>(\\w+)"
            name: "jvm_memory_heap_$1_bytes"
            type: gauge
    "#;

    // ... 테스트 구현
}

#[tokio::test]
async fn test_error_handling_target_down() {
    // 타겟 다운 시 처리 테스트
    let config = create_config_with_invalid_target();

    let mut cmd = Command::cargo_bin("rjmx-exporter").unwrap();
    let mut child = cmd
        .arg("-c")
        .arg(&config.path())
        .spawn()
        .unwrap();

    sleep(Duration::from_millis(100)).await;

    // /metrics 호출 시 에러 메트릭 반환 확인
    let response = reqwest::get("http://localhost:9090/metrics")
        .await
        .unwrap();

    let body = response.text().await.unwrap();
    assert!(body.contains("rjmx_scrape_error"));
}
```

### 5.2 Docker 기반 테스트

#### 테스트 환경 구성

```yaml
# tests/docker-compose.test.yaml
version: '3.8'

services:
  # 테스트용 Java 앱 (Jolokia 포함)
  java-app:
    build:
      context: ./fixtures
      dockerfile: Dockerfile.java-app
    ports:
      - "8778:8778"
    healthcheck:
      test: ["CMD", "curl", "-f", "http://localhost:8778/jolokia/version"]
      interval: 5s
      timeout: 3s
      retries: 5

  # rJMX-Exporter
  rjmx-exporter:
    build:
      context: ../..
      dockerfile: Dockerfile
    ports:
      - "9090:9090"
    volumes:
      - ./fixtures/config.yaml:/config.yaml:ro
    command: ["/rjmx-exporter", "-c", "/config.yaml"]
    depends_on:
      java-app:
        condition: service_healthy

  # 테스트 러너
  test-runner:
    image: curlimages/curl:latest
    depends_on:
      - rjmx-exporter
    command: |
      sh -c '
        sleep 2 &&
        curl -f http://rjmx-exporter:9090/metrics &&
        echo "Test passed!"
      '
```

#### Java 테스트 앱

```dockerfile
# tests/fixtures/Dockerfile.java-app
FROM openjdk:17-slim

# Jolokia 에이전트 다운로드
ADD https://repo1.maven.org/maven2/org/jolokia/jolokia-agent-jvm/2.0.2/jolokia-agent-jvm-2.0.2-javaagent.jar /jolokia-agent.jar

# 간단한 Java 앱
COPY TestApp.java /
RUN javac TestApp.java

EXPOSE 8778

CMD ["java", "-javaagent:/jolokia-agent.jar=port=8778,host=0.0.0.0", "TestApp"]
```

```java
// tests/fixtures/TestApp.java
public class TestApp {
    public static void main(String[] args) throws Exception {
        System.out.println("Test app started");
        // 메모리 사용량 생성
        byte[] data = new byte[10 * 1024 * 1024]; // 10MB
        while (true) {
            Thread.sleep(1000);
        }
    }
}
```

#### Docker 테스트 스크립트

```bash
#!/bin/bash
# scripts/test-docker.sh

set -e

echo "=== Starting Docker Integration Tests ==="

# 이전 컨테이너 정리
docker-compose -f tests/docker-compose.test.yaml down -v

# 테스트 환경 시작
docker-compose -f tests/docker-compose.test.yaml up --build --abort-on-container-exit

# 결과 확인
exit_code=$?
docker-compose -f tests/docker-compose.test.yaml down -v

exit $exit_code
```

### 5.3 assert_cmd를 이용한 CLI 테스트

#### CLI 테스트 예시

```rust
// tests/cli_test.rs
use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::NamedTempFile;
use std::io::Write;

#[test]
fn test_help_flag() {
    let mut cmd = Command::cargo_bin("rjmx-exporter").unwrap();
    cmd.arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("Usage:"))
        .stdout(predicate::str::contains("--config"));
}

#[test]
fn test_version_flag() {
    let mut cmd = Command::cargo_bin("rjmx-exporter").unwrap();
    cmd.arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::contains(env!("CARGO_PKG_VERSION")));
}

#[test]
fn test_validate_valid_config() {
    let config = r#"
jolokia:
  url: "http://localhost:8778/jolokia"
server:
  port: 9090
rules:
  - pattern: "java.lang<type=Memory><HeapMemoryUsage>(\\w+)"
    name: "jvm_memory_heap_$1_bytes"
    type: gauge
"#;

    let mut file = NamedTempFile::new().unwrap();
    file.write_all(config.as_bytes()).unwrap();

    let mut cmd = Command::cargo_bin("rjmx-exporter").unwrap();
    cmd.arg("--validate")
        .arg("-c")
        .arg(file.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("Configuration is valid"));
}

#[test]
fn test_validate_invalid_config() {
    let config = r#"
jolokia:
  url: ""  # 빈 URL
rules:
  - pattern: "[invalid regex"  # 잘못된 정규식
    name: "test"
"#;

    let mut file = NamedTempFile::new().unwrap();
    file.write_all(config.as_bytes()).unwrap();

    let mut cmd = Command::cargo_bin("rjmx-exporter").unwrap();
    cmd.arg("--validate")
        .arg("-c")
        .arg(file.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("Invalid regex"));
}

#[test]
fn test_dry_run() {
    let config = create_valid_config();

    let mut cmd = Command::cargo_bin("rjmx-exporter").unwrap();
    cmd.arg("--dry-run")
        .arg("-c")
        .arg(&config.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("Dry run completed"))
        .stdout(predicate::str::contains("Loaded 1 rule(s)"));
}

#[test]
fn test_missing_config_file() {
    let mut cmd = Command::cargo_bin("rjmx-exporter").unwrap();
    cmd.arg("-c")
        .arg("/nonexistent/config.yaml")
        .assert()
        .failure()
        .stderr(predicate::str::contains("Config file not found"));
}
```

---

## 6. CLI 완성 (CLI Completion)

### 6.1 clap derive 구현

```rust
// src/cli.rs
use clap::{Parser, ValueEnum};
use std::path::PathBuf;

/// rJMX-Exporter - High-performance JMX Metric Exporter
#[derive(Parser, Debug)]
#[command(name = "rjmx-exporter")]
#[command(author, version, about, long_about = None)]
#[command(propagate_version = true)]
pub struct Cli {
    /// Path to configuration file
    #[arg(short, long, value_name = "FILE", default_value = "config.yaml")]
    pub config: PathBuf,

    /// Server port (overrides config file)
    #[arg(short, long, value_name = "PORT")]
    pub port: Option<u16>,

    /// Validate configuration without starting server
    #[arg(long)]
    pub validate: bool,

    /// Test configuration and show parsed rules
    #[arg(long)]
    pub dry_run: bool,

    /// Log level
    #[arg(short, long, value_enum, default_value = "info")]
    pub log_level: LogLevel,

    /// Output format for --validate and --dry-run
    #[arg(long, value_enum, default_value = "text")]
    pub output_format: OutputFormat,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, ValueEnum)]
pub enum LogLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}

impl std::fmt::Display for LogLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LogLevel::Trace => write!(f, "trace"),
            LogLevel::Debug => write!(f, "debug"),
            LogLevel::Info => write!(f, "info"),
            LogLevel::Warn => write!(f, "warn"),
            LogLevel::Error => write!(f, "error"),
        }
    }
}

impl From<LogLevel> for tracing::Level {
    fn from(level: LogLevel) -> Self {
        match level {
            LogLevel::Trace => tracing::Level::TRACE,
            LogLevel::Debug => tracing::Level::DEBUG,
            LogLevel::Info => tracing::Level::INFO,
            LogLevel::Warn => tracing::Level::WARN,
            LogLevel::Error => tracing::Level::ERROR,
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, ValueEnum)]
pub enum OutputFormat {
    Text,
    Json,
    Yaml,
}
```

### 6.2 CLI 옵션 상세

| 옵션 | 단축 | 설명 | 기본값 |
|------|------|------|--------|
| `--config` | `-c` | 설정 파일 경로 | `config.yaml` |
| `--port` | `-p` | 서버 포트 (설정 파일 오버라이드) | 설정 파일 값 |
| `--validate` | - | 설정 검증만 수행 | - |
| `--dry-run` | - | 설정 테스트 및 파싱된 규칙 출력 | - |
| `--log-level` | `-l` | 로그 레벨 | `info` |
| `--output-format` | - | 출력 형식 (text/json/yaml) | `text` |
| `--help` | `-h` | 도움말 출력 | - |
| `--version` | `-V` | 버전 출력 | - |

### 6.3 main.rs 구현

```rust
// src/main.rs
use clap::Parser;
use tracing_subscriber::EnvFilter;

mod cli;
mod config;
mod collector;
mod transformer;
mod server;
mod error;

use cli::Cli;
use config::Config;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    // 로깅 설정
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(cli.log_level.to_string()));

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .init();

    // 설정 로드
    let config = Config::load(&cli.config)?;

    // --validate 모드
    if cli.validate {
        return validate_config(&config, &cli);
    }

    // --dry-run 모드
    if cli.dry_run {
        return dry_run(&config, &cli);
    }

    // 포트 오버라이드
    let port = cli.port.unwrap_or(config.server.port);

    // 서버 시작
    tracing::info!(port = port, "Starting rJMX-Exporter");
    server::run(config, port).await
}

fn validate_config(config: &Config, cli: &Cli) -> anyhow::Result<()> {
    // 규칙 컴파일 테스트 (Java → Rust regex 변환 적용)
    for (i, rule) in config.rules.iter().enumerate() {
        // Use the same conversion logic as Phase 3 TransformEngine
        let converted_pattern = convert_java_regex(&rule.pattern)
            .map_err(|e| anyhow::anyhow!("Rule {}: {}", i, e))?;

        if let Err(e) = regex::Regex::new(&converted_pattern) {
            tracing::error!(
                rule = i,
                original_pattern = &rule.pattern,
                converted_pattern = &converted_pattern,
                "Invalid regex: {}", e
            );
            anyhow::bail!("Invalid regex in rule {}: {} (converted: {})", i, e, converted_pattern);
        }
    }

    match cli.output_format {
        cli::OutputFormat::Text => {
            println!("Configuration is valid");
            println!("  Jolokia URL: {}", config.jolokia.url);
            println!("  Rules: {}", config.rules.len());
        }
        cli::OutputFormat::Json => {
            let result = serde_json::json!({
                "valid": true,
                "jolokia_url": config.jolokia.url,
                "rules_count": config.rules.len()
            });
            println!("{}", serde_json::to_string_pretty(&result)?);
        }
        cli::OutputFormat::Yaml => {
            // YAML 출력
        }
    }

    Ok(())
}

fn dry_run(config: &Config, cli: &Cli) -> anyhow::Result<()> {
    println!("Dry run completed");
    println!("Loaded {} rule(s)", config.rules.len());

    for (i, rule) in config.rules.iter().enumerate() {
        println!("\nRule {}:", i + 1);
        println!("  Pattern: {}", rule.pattern);
        println!("  Name: {}", rule.name);
        println!("  Type: {:?}", rule.metric_type);
    }

    Ok(())
}
```

---

## 7. 문서화 (Documentation)

### 7.1 README 업데이트 체크리스트

- [ ] 설치 방법 (바이너리 다운로드, cargo install)
- [ ] 빠른 시작 가이드
- [ ] 설정 옵션 전체 목록
- [ ] CLI 옵션 설명
- [ ] 예제 설정 파일
- [ ] Prometheus 설정 예제
- [ ] 트러블슈팅 가이드
- [ ] FAQ

### 7.2 사용 예제

```markdown
## Quick Start

### 1. Install Jolokia on your Java app

```bash
java -javaagent:jolokia-agent-jvm.jar=port=8778,host=0.0.0.0 -jar your-app.jar
```

### 2. Create configuration file

```yaml
# config.yaml
jolokia:
  url: "http://localhost:8778/jolokia"

server:
  port: 9090
  path: "/metrics"

rules:
  - pattern: "java.lang<type=Memory><HeapMemoryUsage>(\\w+)"
    name: "jvm_memory_heap_$1_bytes"
    type: gauge
```

### 3. Run rJMX-Exporter

```bash
./rjmx-exporter -c config.yaml
```

### 4. Verify metrics

```bash
curl http://localhost:9090/metrics
```
```

### 7.3 jmx_exporter 마이그레이션 가이드

```markdown
## Migration from jmx_exporter

### Step 1: Install Jolokia

If you're using jmx_exporter in agent mode, you need to add Jolokia:

```bash
# Before (jmx_exporter agent)
java -javaagent:jmx_prometheus_javaagent.jar=9090:config.yaml -jar app.jar

# After (Jolokia + rJMX-Exporter)
java -javaagent:jolokia-agent-jvm.jar=port=8778 -jar app.jar
```

### Step 2: Update configuration

Add the `jolokia` block to your existing config:

```diff
+ jolokia:
+   url: "http://localhost:8778/jolokia"
+
+ server:
+   port: 9090

  lowercaseOutputName: true
  rules:
    - pattern: "..."
      name: "..."
```

### Step 3: Compatibility check

```bash
# Validate your config
./rjmx-exporter --validate -c config.yaml

# Test with dry-run
./rjmx-exporter --dry-run -c config.yaml
```

### Unsupported Options

| Option | Alternative |
|--------|-------------|
| `hostPort` | Use `jolokia.url` |
| `jmxUrl` | Jolokia only |
| `ssl` | Use `https://` in URL |
```

### 7.4 rustdoc 문서화

```rust
//! # rJMX-Exporter
//!
//! A high-performance JMX metric exporter for Prometheus, written in Rust.
//!
//! ## Features
//!
//! - Native binary (no JVM required)
//! - Memory usage < 10MB
//! - Startup time < 100ms
//! - Scrape latency < 10ms
//!
//! ## Example
//!
//! ```rust,no_run
//! use rjmx_exporter::{Config, Server};
//!
//! #[tokio::main]
//! async fn main() -> anyhow::Result<()> {
//!     let config = Config::load("config.yaml")?;
//!     Server::new(config).run().await
//! }
//! ```

/// Configuration for the exporter.
///
/// # Example
///
/// ```yaml
/// jolokia:
///   url: "http://localhost:8778/jolokia"
/// server:
///   port: 9090
/// ```
pub struct Config {
    // ...
}
```

---

## 8. 릴리스 준비 (Release Preparation)

### 8.1 GitHub Actions CI/CD

```yaml
# .github/workflows/ci.yaml
name: CI

on:
  push:
    branches: [main]
  pull_request:
    branches: [main]

env:
  CARGO_TERM_COLOR: always

jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Install Rust
        uses: dtolnay/rust-action@stable

      - name: Cache cargo
        uses: Swatinem/rust-cache@v2

      - name: Run tests
        run: cargo test --all-features

      - name: Run clippy
        run: cargo clippy -- -D warnings

      - name: Check formatting
        run: cargo fmt -- --check

  integration-test:
    runs-on: ubuntu-latest
    needs: test
    steps:
      - uses: actions/checkout@v4

      - name: Set up Docker Buildx
        uses: docker/setup-buildx-action@v3

      - name: Run integration tests
        run: ./scripts/test-docker.sh

  benchmark:
    runs-on: ubuntu-latest
    needs: test
    steps:
      - uses: actions/checkout@v4

      - name: Install Rust
        uses: dtolnay/rust-action@stable

      - name: Run benchmarks
        run: cargo bench --no-run
```

### 8.2 릴리스 워크플로우

```yaml
# .github/workflows/release.yaml
name: Release

on:
  push:
    tags:
      - 'v*'

jobs:
  build:
    strategy:
      matrix:
        include:
          - target: x86_64-unknown-linux-gnu
            os: ubuntu-latest
            name: linux-amd64
          - target: x86_64-unknown-linux-musl
            os: ubuntu-latest
            name: linux-amd64-musl
          - target: aarch64-unknown-linux-gnu
            os: ubuntu-latest
            name: linux-arm64
          - target: x86_64-apple-darwin
            os: macos-latest
            name: darwin-amd64
          - target: aarch64-apple-darwin
            os: macos-latest
            name: darwin-arm64
          - target: x86_64-pc-windows-msvc
            os: windows-latest
            name: windows-amd64

    runs-on: ${{ matrix.os }}
    steps:
      - uses: actions/checkout@v4

      - name: Install Rust
        uses: dtolnay/rust-action@stable
        with:
          targets: ${{ matrix.target }}

      - name: Build
        run: cargo build --release --target ${{ matrix.target }}

      - name: Package (Unix)
        if: matrix.os != 'windows-latest'
        run: |
          cd target/${{ matrix.target }}/release
          tar czvf rjmx-exporter-${{ matrix.name }}.tar.gz rjmx-exporter

      - name: Package (Windows)
        if: matrix.os == 'windows-latest'
        run: |
          cd target/${{ matrix.target }}/release
          7z a rjmx-exporter-${{ matrix.name }}.zip rjmx-exporter.exe

      - name: Upload artifact
        uses: actions/upload-artifact@v4
        with:
          name: rjmx-exporter-${{ matrix.name }}
          path: target/${{ matrix.target }}/release/rjmx-exporter-*

  docker:
    runs-on: ubuntu-latest
    needs: build
    steps:
      - uses: actions/checkout@v4

      - name: Set up Docker Buildx
        uses: docker/setup-buildx-action@v3

      - name: Login to Docker Hub
        uses: docker/login-action@v3
        with:
          username: ${{ secrets.DOCKERHUB_USERNAME }}
          password: ${{ secrets.DOCKERHUB_TOKEN }}

      - name: Build and push
        uses: docker/build-push-action@v5
        with:
          context: .
          push: true
          tags: |
            rjmx/exporter:${{ github.ref_name }}
            rjmx/exporter:latest
          platforms: linux/amd64,linux/arm64

  release:
    runs-on: ubuntu-latest
    needs: [build, docker]
    steps:
      - name: Download artifacts
        uses: actions/download-artifact@v4

      - name: Create Release
        uses: softprops/action-gh-release@v1
        with:
          files: |
            rjmx-exporter-*/*
          generate_release_notes: true
```

### 8.3 Docker 이미지

```dockerfile
# Dockerfile
# Stage 1: Build
FROM rust:1.75-alpine AS builder

RUN apk add --no-cache musl-dev

WORKDIR /app
COPY . .

RUN cargo build --release --target x86_64-unknown-linux-musl

# Stage 2: Runtime
FROM alpine:3.19

RUN apk add --no-cache ca-certificates

COPY --from=builder /app/target/x86_64-unknown-linux-musl/release/rjmx-exporter /usr/local/bin/

EXPOSE 9090

ENTRYPOINT ["/usr/local/bin/rjmx-exporter"]
CMD ["-c", "/config.yaml"]
```

```yaml
# docker-compose.yaml (예제)
version: '3.8'

services:
  rjmx-exporter:
    image: rjmx/exporter:latest
    ports:
      - "9090:9090"
    volumes:
      - ./config.yaml:/config.yaml:ro
    restart: unless-stopped
```

### 8.4 멀티 플랫폼 빌드

| 플랫폼 | Target | 비고 |
|--------|--------|------|
| Linux (x64) | `x86_64-unknown-linux-gnu` | glibc 기반 |
| Linux (x64, static) | `x86_64-unknown-linux-musl` | 정적 링크, Alpine |
| Linux (ARM64) | `aarch64-unknown-linux-gnu` | AWS Graviton, Apple Silicon VM |
| macOS (x64) | `x86_64-apple-darwin` | Intel Mac |
| macOS (ARM64) | `aarch64-apple-darwin` | Apple Silicon |
| Windows (x64) | `x86_64-pc-windows-msvc` | Windows 10+ |

---

## 9. 완료 기준 (Definition of Done)

### 9.1 기능 완료 기준

| 항목 | 완료 조건 | 검증 방법 |
|------|-----------|-----------|
| 메트릭 수집 | Jolokia에서 JMX 메트릭 수집 | 통합 테스트 |
| 규칙 변환 | pattern → Prometheus 포맷 변환 | 단위 테스트 |
| 레이블 처리 | 정적/동적 레이블 지원 | 단위 테스트 |
| CLI | 모든 옵션 동작 | CLI 테스트 |
| 에러 처리 | graceful 에러 처리 | 통합 테스트 |
| 로깅 | structured logging 동작 | 수동 검증 |

### 9.2 성능 완료 기준

| 항목 | 목표 | 검증 방법 |
|------|------|-----------|
| 메모리 | < 10MB | 벤치마크 |
| 시작 시간 | < 100ms | 벤치마크 |
| 스크레이프 지연 | < 10ms (P99) | 벤치마크 |
| 바이너리 크기 | < 10MB | 빌드 확인 |

### 9.3 품질 완료 기준

| 항목 | 목표 | 검증 방법 |
|------|------|-----------|
| 테스트 커버리지 | > 80% | `cargo tarpaulin` |
| Clippy 경고 | 0개 | `cargo clippy -- -D warnings` |
| 문서화 | 모든 pub 항목 | `cargo doc --no-deps` |
| 포맷팅 | 통과 | `cargo fmt -- --check` |

### 9.4 릴리스 완료 기준

| 항목 | 완료 조건 |
|------|-----------|
| CI/CD | 모든 PR에서 자동 테스트 |
| 바이너리 | 6개 플랫폼 빌드 성공 |
| Docker | 이미지 빌드 및 푸시 성공 |
| 문서 | README 및 마이그레이션 가이드 완성 |
| 릴리스 노트 | 변경 사항 문서화 |

---

## 10. 성능 목표 체크리스트 (Performance Target Checklist)

### 10.1 메모리 목표 (< 10MB)

- [ ] Idle 상태 메모리 측정: _____ MB
- [ ] 단일 스크레이프 메모리 측정: _____ MB
- [ ] 10 동시 스크레이프 메모리 측정: _____ MB
- [ ] 1000+ 메트릭 처리 시 메모리 측정: _____ MB
- [ ] jmx_exporter 대비 메모리 절감률: _____ %

### 10.2 시작 시간 목표 (< 100ms)

- [ ] 설정 로드 시간: _____ ms
- [ ] Regex 컴파일 시간: _____ ms
- [ ] 서버 바인딩 시간: _____ ms
- [ ] 총 시작 시간: _____ ms
- [ ] hyperfine 측정 결과 첨부

### 10.3 스크레이프 지연 목표 (< 10ms)

- [ ] Jolokia 요청 시간 (P50): _____ ms
- [ ] Jolokia 요청 시간 (P99): _____ ms
- [ ] 전체 스크레이프 지연 (P50): _____ ms
- [ ] 전체 스크레이프 지연 (P99): _____ ms
- [ ] wrk 부하 테스트 결과 첨부

### 10.4 바이너리 크기 목표 (< 10MB)

- [ ] 기본 release 빌드: _____ MB
- [ ] LTO 적용 후: _____ MB
- [ ] strip 적용 후: _____ MB
- [ ] 최종 바이너리 크기: _____ MB

### 10.5 비교 테스트 결과

| 항목 | jmx_exporter | rJMX-Exporter | 개선율 |
|------|--------------|---------------|--------|
| 메모리 (RSS) | _____ MB | _____ MB | _____ % |
| 시작 시간 | _____ ms | _____ ms | _____ % |
| P50 지연 | _____ ms | _____ ms | _____ % |
| P99 지연 | _____ ms | _____ ms | _____ % |
| 처리량 (req/s) | _____ | _____ | _____ % |

---

## 11. 타임라인 (Timeline)

| 주차 | 작업 내용 |
|------|-----------|
| Week 1 | 벤치마크 환경 구축, 기준 측정 |
| Week 2 | 성능 최적화 (Zero-copy, Pre-compiled regex) |
| Week 3 | 통합 테스트 작성, Docker 테스트 환경 |
| Week 4 | CLI 완성, 문서화 |
| Week 5 | CI/CD 파이프라인, 멀티 플랫폼 빌드 |
| Week 6 | 최종 검증, 릴리스 |

---

## 12. 참고 자료 (References)

- [Criterion.rs 벤치마킹](https://bheisler.github.io/criterion.rs/book/)
- [Rust 성능 최적화](https://nnethercote.github.io/perf-book/)
- [Serde 성능 팁](https://serde.rs/attr-borrow.html)
- [Tokio 성능 튜닝](https://tokio.rs/tokio/topics/bridging)
- [GitHub Actions Rust](https://github.com/actions-rs)
- [Cross-compilation](https://rust-lang.github.io/rustup/cross-compilation.html)
