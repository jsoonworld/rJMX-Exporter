# [Project 1-Pager] Rust 기반 고성능 JMX Metric Exporter

## 1. 프로젝트 개요 (Overview)

- **프로젝트 명:** `rJMX-Exporter` (Rust-based JMX Exporter)
- **한 줄 요약:** Java 애플리케이션의 리소스 간섭을 최소화하기 위해 Rust로 재작성한 초경량 고성능 JMX 메트릭 수집기 및 Prometheus Exporter.
- **배경:** 기존 Java 기반 `jmx_exporter`는 대상 앱의 JVM 힙 메모리를 공유하여 OOM 리스크를 높이고 GC 부하를 유발함. 이를 Rust 사이드카 구조로 변경하여 인프라 효율성을 극대화함.

## 2. 목표 및 핵심 가치 (Goals & Value)

- **Resource Efficiency:** JVM 없이 실행되어 메모리 사용량 90% 이상 절감 (예: 50MB -> 5MB 미만).
- **Zero Interference:** 대상 애플리케이션의 GC 및 런타임 성능에 영향을 주지 않음.
- **High Performance:** Rust의 비동기 I/O(`Tokio`)를 활용하여 수천 개의 메트릭 파싱 및 서빙 지연 시간(Latency) 최소화.

## 3. 기술 스택 (Tech Stack)

| Category | Technology |
|----------|------------|
| Language | Rust (Edition 2021) |
| Async Runtime | `Tokio` |
| HTTP Server | `Axum` (Prometheus 엔드포인트 서빙) |
| HTTP Client | `Reqwest` (JMX 데이터 수집) |
| Serialization | `Serde`, `Serde_yaml` (설정 파일 로드) |
| Observability | `Prometheus` crate, `Tracing` (로깅) |
| Target Interface | Jolokia (JMX-over-HTTP) |

## 4. 시스템 아키텍처 (Architecture)

```
┌─────────────────┐     ┌─────────────────┐     ┌─────────────────┐
│   Java App      │     │  rJMX-Exporter  │     │   Prometheus    │
│  (Spring Boot)  │     │     (Rust)      │     │                 │
│                 │     │                 │     │                 │
│  ┌───────────┐  │     │  ┌───────────┐  │     │                 │
│  │  Jolokia  │◄─┼─────┼──┤ Collector │  │     │                 │
│  │  Agent    │  │JSON │  └─────┬─────┘  │     │                 │
│  └───────────┘  │     │        │        │     │                 │
│                 │     │  ┌─────▼─────┐  │     │                 │
│                 │     │  │Transformer│  │     │                 │
│                 │     │  └─────┬─────┘  │     │                 │
│                 │     │        │        │     │                 │
│                 │     │  ┌─────▼─────┐  │GET  │                 │
│                 │     │  │  /metrics │◄─┼─────┤  Scraper        │
│                 │     │  └───────────┘  │     │                 │
└─────────────────┘     └─────────────────┘     └─────────────────┘
```

### 컴포넌트 설명

1. **Java App:** 메트릭을 노출하는 대상 (Spring Boot 등).
2. **JMX Bridge (Jolokia):** Java 앱 내부에 위치하며 JMX를 HTTP/JSON으로 변환.
3. **rJMX-Exporter (Rust):**
   - **Collector:** 주기적으로 Jolokia에서 JSON 데이터를 Fetch.
   - **Transformer:** 기존 `jmx_exporter`의 YAML 규칙을 해석하여 Prometheus 포맷으로 변환.
   - **Registry:** 변환된 데이터를 메모리에 캐싱.
4. **Prometheus:** `rJMX-Exporter`가 열어둔 포트(`/metrics`)에서 데이터를 스크래핑.

## 5. 단계별 구현 계획 (Roadmap)

### Phase 1: 기초 환경 구축
- [ ] Docker를 활용한 샘플 Java 앱 및 Prometheus 환경 구성
- [ ] Rust 프로젝트 초기 설정 및 `Tokio`/`Axum` 서버 베이스라인 구축

### Phase 2: 데이터 수집 및 파싱
- [ ] Jolokia 엔드포인트로부터 JSON 메트릭 수집 로직 구현
- [ ] `Serde`를 이용한 동적 데이터 구조 매핑

### Phase 3: 매핑 엔진(Rule Engine) 구현
- [ ] 기존 Java JMX Exporter의 YAML 설정 형식을 지원하는 매핑 로직 작성
- [ ] Regex 기반 메트릭 이름 변환

### Phase 4: 성능 검증
- [ ] Java 버전 vs Rust 버전의 리소스 점유율 비교 벤치마크 수행
- [ ] 메모리 사용량 및 응답 지연 시간 측정

## 6. 레포지토리 구조 (Repository Structure)

```
rJMX-Exporter/
├── Cargo.toml           # Rust 프로젝트 설정
├── README.md            # 프로젝트 소개
├── docs/
│   └── 1-PAGER.md       # 프로젝트 1-Pager (본 문서)
├── src/
│   ├── main.rs          # 엔트리 포인트
│   ├── collector.rs     # JMX 데이터 수집 로직
│   ├── transformer.rs   # 메트릭 변환 엔진 (핵심)
│   ├── config.rs        # YAML 설정 파서
│   └── server.rs        # Prometheus HTTP 서버
├── tests/               # 성능 테스트 및 유닛 테스트
└── example/             # 샘플 Java 앱 및 Docker-compose
```

## 7. 성공 지표 (Success Metrics)

| Metric | Target |
|--------|--------|
| 메모리 사용량 | < 10MB (vs Java 50MB+) |
| 시작 시간 | < 100ms |
| 메트릭 수집 지연 | < 10ms per scrape |
| CPU 사용률 | < 1% idle |

## 8. 참고 자료 (References)

- [Prometheus JMX Exporter (Java)](https://github.com/prometheus/jmx_exporter)
- [Jolokia - JMX-HTTP Bridge](https://jolokia.org/)
- [Tokio - Rust Async Runtime](https://tokio.rs/)
- [Axum - Web Framework](https://github.com/tokio-rs/axum)

---

**Repository:** https://github.com/jsoonworld/rJMX-Exporter
