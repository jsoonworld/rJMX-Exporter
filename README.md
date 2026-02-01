# rJMX-Exporter

Rust-based high-performance JMX Metric Exporter for Prometheus

## Overview

`rJMX-Exporter`는 Java 애플리케이션의 JMX 메트릭을 Prometheus 포맷으로 변환하는 초경량 고성능 사이드카입니다.

기존 Java 기반 `jmx_exporter`와 달리 JVM 없이 실행되어:
- 메모리 사용량 90% 이상 절감 (50MB -> 5MB 미만)
- 대상 애플리케이션의 GC 및 런타임에 영향 없음
- 빠른 시작 시간 및 낮은 지연 시간

## Quick Start

```bash
# Build
cargo build --release

# Run
./target/release/rjmx-exporter --config config.yaml
```

## Architecture

```
Java App (Jolokia) --> rJMX-Exporter (Rust) --> Prometheus
```

## Configuration

```yaml
# config.yaml
targets:
  - url: "http://localhost:8778/jolokia"
    name: "my-java-app"

server:
  port: 9090

rules:
  - pattern: "java.lang<type=Memory><HeapMemoryUsage>(\\w+)"
    name: "jvm_memory_heap_$1_bytes"
    type: gauge
```

## Documentation

- [1-Pager (프로젝트 상세)](docs/1-PAGER.md)

## License

MIT
