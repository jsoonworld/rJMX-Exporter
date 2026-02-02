# rJMX-Exporter Design Document

> **Status: Phase 1 Foundation implemented (as of 2026-02-01)** - Phase 2 design in progress.

## 1. Overview

**Project:** rJMX-Exporter (Rust-based JMX Exporter)

**Summary:** A Rust-based sidecar that collects JMX metrics from JVM applications and exports them in Prometheus format.

**Problem:** The official [jmx_exporter](https://github.com/prometheus/jmx_exporter) has two modes, both with drawbacks:

| Mode | How it works | Issues |
|------|--------------|--------|
| javaagent | Runs inside target JVM | Shares heap/GC, can cause OOM, adds latency |
| HTTP server | Separate JVM process | ~50MB+ memory, requires JVM management |

**Solution:** rJMX-Exporter runs as a native Rust binary, collecting metrics via Jolokia (JMX-over-HTTP). This provides complete isolation from the target application with minimal resource usage.

## 2. Goals

| Metric | Target | Notes |
|--------|--------|-------|
| Memory Usage | <10MB | 80%+ reduction vs Java standalone |
| Startup Time | <100ms | |
| Scrape Latency | <10ms | Collection to response |
| Impact on Target | 0% | Isolated process |

## 3. Non-Goals (v1)

- No in-process JVM agent mode (sidecar only)
- No direct JMX/RMI collection (Jolokia required)
- No auto-discovery (explicit targets config)
- Metrics-only HTTP endpoint (no UI)

## 4. Architecture

```text
┌─────────────────┐     ┌─────────────────┐     ┌─────────────────┐
│    JVM App      │     │  rJMX-Exporter  │     │   Prometheus    │
│                 │     │     (Rust)      │     │                 │
│  ┌───────────┐  │     │                 │     │                 │
│  │  Jolokia  │◄─┼─────┤  Collector      │     │                 │
│  │  Agent    │  │JSON │       ↓         │     │                 │
│  └───────────┘  │     │  Transformer    │     │                 │
│                 │     │       ↓         │     │                 │
│                 │     │  /metrics  ◄────┼─────┤  Scraper        │
└─────────────────┘     └─────────────────┘     └─────────────────┘
```

### Components

| Component | Role |
|-----------|------|
| Collector | Fetches JSON metrics from Jolokia endpoint |
| Transformer | Converts MBean data → Prometheus format |
| Server | Serves `/metrics` endpoint |

### Prerequisites

- **Jolokia**: Must be installed on the JVM app as a JVM agent or WAR
- **Prometheus**: Configured to scrape rJMX-Exporter's `/metrics` endpoint
- **Network access**: Exporter must reach the Jolokia endpoint (auth/TLS handling TBD)

## 5. Tech Stack

| Category | Technology | Purpose |
|----------|------------|---------|
| Language | Rust 2021 | |
| Async Runtime | Tokio | Async I/O |
| HTTP Server | Axum | `/metrics` endpoint |
| HTTP Client | Reqwest | Jolokia data collection |
| Serialization | Serde, serde_yaml | Config files, JSON parsing |
| Logging | tracing | Structured logging |

## 6. Configuration (Planned)

```yaml
jolokia:
  url: "http://localhost:8778/jolokia"
  # username: "user"    # optional
  # password: "pass"    # optional
  # timeout_ms: 5000    # optional

server:
  port: 9090
  path: "/metrics"

rules:
  - pattern: "java.lang<type=Memory><HeapMemoryUsage>(\\w+)"
    name: "jvm_memory_heap_$1_bytes"
    type: gauge
```

### jmx_exporter Compatibility

**Design Principle:** Existing jmx_exporter config files should work with minimal changes (add `jolokia:` block only).

#### Supported Options (Planned)

| Option | Priority | Notes |
|--------|----------|-------|
| `rules[].pattern` | P0 | MBean regex matching |
| `rules[].name` | P0 | Prometheus metric name with `$1`, `$2` capture groups |
| `rules[].type` | P0 | `gauge`, `counter`, `untyped` |
| `rules[].labels` | P0 | Static and dynamic (`$1`) labels |
| `rules[].help` | P1 | Metric help text |
| `whitelistObjectNames` | P1 | MBean filter (glob patterns) |
| `blacklistObjectNames` | P1 | MBean exclusion filter |
| `lowercaseOutputName` | P2 | Lowercase metric names |
| `lowercaseOutputLabelNames` | P2 | Lowercase label names |
| `rules[].value` | P2 | Custom value expression |
| `rules[].valueFactor` | P2 | Multiply value (e.g., ms→s) |

#### Not Planned (v1)

| Option | Reason |
|--------|--------|
| `hostPort` | Use `jolokia.url` instead |
| `jmxUrl` | Jolokia only, no direct RMI |
| `ssl` / `sslConfig` | Use `jolokia.url` with https |
| `rules[].attrNameSnakeCase` | Low demand, easy workaround |

#### Migration Example

```yaml
# Before (jmx_exporter standalone)
hostPort: localhost:9999

# After (rJMX-Exporter)
jolokia:
  url: "http://localhost:8778/jolokia"

# Rest of config unchanged
lowercaseOutputName: true
rules:
  - pattern: "..."
```

## 7. Operational Considerations (Planned)

- Jolokia request timeouts/retries and overall scrape deadlines
- Partial scrape behavior and exporter self-metrics (errors, durations)
- Label cardinality guardrails and rule allowlists
- Optional caching vs live fetch tradeoffs

## 8. Migration Tools (Planned)

### Config Validator
```bash
# Validate existing jmx_exporter config
rjmx-exporter validate -c config.yaml

# Output:
# ✓ rules[0].pattern - supported
# ✓ rules[0].name - supported
# ⚠ rules[2].attrNameSnakeCase - not supported (ignored)
# ✗ hostPort - use jolokia.url instead
```

### Dry Run Mode
```bash
# Test config without starting server
rjmx-exporter --dry-run -c config.yaml

# Shows:
# - Parsed rules
# - Sample metric output
# - Warnings for unsupported options
```

## 9. Open Questions

- Should config reload be supported (SIGHUP / hot reload)?
- Should multi-target mode be first-class or strictly sidecar?
- Which Jolokia auth modes should be supported (basic auth, bearer, TLS)?
- How should complex JMX types/arrays be mapped into Prometheus metrics?
- Should we support jmx_exporter `includeObjectNames` as alias for `whitelistObjectNames`?

## 10. Roadmap

### Phase 1: Foundation
- [ ] Create Rust project structure (Cargo.toml, src/)
- [ ] Basic Tokio + Axum server
- [ ] Sample Java app + Jolokia Docker environment

### Phase 2: Data Collection
- [ ] Parse Jolokia JSON responses
- [ ] Define MBean data structures

### Phase 3: Transform Engine
- [ ] YAML rule parser
- [ ] Regex-based metric name transformation
- [ ] Prometheus text format output

### Phase 4: Validation
- [ ] Benchmark resource usage vs Java version
- [ ] Integration tests

## 11. Repository Structure (Planned)

```
rJMX-Exporter/
├── Cargo.toml
├── src/
│   ├── main.rs
│   ├── lib.rs
│   ├── config.rs
│   ├── error.rs
│   ├── collector/
│   │   ├── mod.rs
│   │   ├── client.rs
│   │   └── parser.rs
│   ├── transformer/
│   │   ├── mod.rs
│   │   └── rules.rs
│   └── server/
│       ├── mod.rs
│       └── handlers.rs
├── tests/
├── examples/
│   └── docker-compose.yaml
└── docs/
    ├── 1-PAGER.md
    └── TECH-STACK.md
```

## 12. References

- [jmx_exporter (Java)](https://github.com/prometheus/jmx_exporter)
- [Jolokia](https://jolokia.org/)
- [Tokio](https://tokio.rs/)
- [Axum](https://github.com/tokio-rs/axum)

---

**Repository:** https://github.com/jsoonworld/rJMX-Exporter
