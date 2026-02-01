# rJMX-Exporter

> **Status: Design Phase** - Implementation not yet started.

A high-performance JMX Metric Exporter for Prometheus, written in Rust.

## Background

[jmx_exporter](https://github.com/prometheus/jmx_exporter) is the official Prometheus exporter for JMX metrics, widely used to monitor JVM applications (Kafka, Cassandra, Spring Boot, etc.).

However, it has architectural limitations:

**javaagent mode** (recommended by maintainers):
- Runs inside the target JVM → shares heap memory and GC
- Can cause OOM when the target app is under memory pressure
- Adds latency during GC pauses
- Must restart with the application

**HTTP server mode** (standalone):
- Requires a separate JVM process → ~50MB+ memory overhead
- Still needs JVM installation and management

**rJMX-Exporter** solves these issues by:
- Running as a native binary (no JVM required)
- Collecting metrics via [Jolokia](https://jolokia.org/) (JMX-over-HTTP)
- Complete isolation from target application

## Goals

| Metric | jmx_exporter (Java) | rJMX-Exporter (Target) |
|--------|---------------------|------------------------|
| Memory Usage | ~50MB (standalone) | <10MB |
| Requires JVM | Yes | No |
| Impact on Target App | Shares GC/Heap (agent mode) | None |
| Startup Time | seconds | <100ms |

## Planned Features

- Native Rust binary (no JVM required)
- Jolokia HTTP/JSON collection (agent or WAR)
- Prometheus text exposition at `/metrics`
- Rule-based mapping with partial `jmx_exporter` YAML compatibility (`pattern`, `name`, `type`, `labels`)
- Resource targets: <10MB memory, <100ms startup, <10ms scrape latency
- Sidecar-first deployment; multi-target configuration under consideration

## Architecture

```
┌─────────────────┐      ┌─────────────────┐      ┌─────────────────┐
│    JVM App      │      │  rJMX-Exporter  │      │   Prometheus    │
│                 │      │     (Rust)      │      │                 │
│  ┌───────────┐  │      │                 │      │                 │
│  │  Jolokia  │◄─┼──────┤  Collector      │      │                 │
│  │  Agent    │  │ JSON │       ↓         │      │                 │
│  └───────────┘  │      │  Transformer    │      │                 │
│                 │      │       ↓         │      │                 │
│                 │      │  /metrics  ◄────┼──────┤  Scraper        │
└─────────────────┘      └─────────────────┘      └─────────────────┘
```

## Installation and Usage (Planned)

1. Install Jolokia on the target JVM (agent or WAR).
2. Create a config file (see below).
3. Run rJMX-Exporter with the config file (CLI flags TBD).
4. Configure Prometheus to scrape the `/metrics` endpoint.

## Migration from jmx_exporter

Designed for easy migration from existing jmx_exporter setups:

```diff
  # Your existing jmx_exporter config
+ jolokia:
+   url: "http://localhost:8778/jolokia"
+
  lowercaseOutputName: true
  whitelistObjectNames:
    - "java.lang:*"
  rules:
    - pattern: "..."
      name: "..."
      type: gauge
```

**3-Step Migration:**
1. Add Jolokia agent to your JVM app: `-javaagent:jolokia-agent.jar=port=8778`
2. Copy your existing config and add the `jolokia:` block
3. Run rJMX-Exporter: `./rjmx-exporter -c config.yaml`

## Configuration (Planned)

```yaml
# rJMX-Exporter specific
jolokia:
  url: "http://localhost:8778/jolokia"
  # username: "user"        # optional
  # password: "pass"        # optional
  # timeout_ms: 5000        # optional

server:
  port: 9090
  path: "/metrics"

# jmx_exporter compatible options
lowercaseOutputName: true
lowercaseOutputLabelNames: true

whitelistObjectNames:
  - "java.lang:*"
  - "java.nio:*"

blacklistObjectNames:
  - "java.lang:type=MemoryPool,*"

rules:
  - pattern: "java.lang<type=Memory><HeapMemoryUsage>(\\w+)"
    name: "jvm_memory_heap_$1_bytes"
    type: gauge
    help: "JVM heap memory usage"
    labels:
      app: "my-app"
```

### Compatibility Matrix

| Option | Priority | Status |
|--------|----------|--------|
| `rules.pattern` | Required | Planned |
| `rules.name` | Required | Planned |
| `rules.type` | Required | Planned |
| `rules.labels` | Required | Planned |
| `rules.help` | High | Planned |
| `whitelistObjectNames` | High | Planned |
| `blacklistObjectNames` | High | Planned |
| `lowercaseOutputName` | Medium | Planned |
| `lowercaseOutputLabelNames` | Medium | Planned |
| `rules.value` | Medium | Planned |
| `rules.valueFactor` | Medium | Planned |

## Prometheus Configuration (Example)

```yaml
scrape_configs:
  - job_name: "rjmx"
    static_configs:
      - targets: ["<host>:9090"]
```

## Tech Stack

- **Rust** (Edition 2021)
- **Tokio** - async runtime
- **Axum** - HTTP server for `/metrics`
- **Reqwest** - HTTP client for Jolokia
- **Serde** - YAML/JSON parsing

## Documentation

- [Design Document (1-Pager)](docs/1-PAGER.md)
- [Technology Stack](docs/TECH-STACK.md)

## Contributing

Issues and PRs are welcome. For now, please check the design doc and open questions first.

## License

MIT
