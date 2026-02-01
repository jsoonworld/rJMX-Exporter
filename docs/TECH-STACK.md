# Technology Stack

> rJMX-Exporter technology stack and selection rationale

## 1. Overview

```
┌─────────────────────────────────────────────────────────────────────────┐
│                          rJMX-Exporter                                  │
├─────────────────────────────────────────────────────────────────────────┤
│  HTTP Server Layer     │  Axum + Tower                                  │
│  Async Runtime         │  Tokio                                         │
│  HTTP Client           │  Reqwest                                       │
│  Serialization         │  Serde (JSON, YAML)                            │
│  Pattern Matching      │  Regex                                         │
│  Logging               │  tracing + tracing-subscriber                  │
│  Error Handling        │  thiserror + anyhow                            │
│  CLI                   │  clap                                          │
└─────────────────────────────────────────────────────────────────────────┘
```

## 2. Core Dependencies

### 2.1 Async Runtime: Tokio

| Aspect | Detail |
|--------|--------|
| Crate | `tokio` |
| Version | 1.x |
| Features | `rt-multi-thread`, `macros`, `net`, `time`, `signal` |

**Why Tokio?**
- De facto standard in Rust async ecosystem
- Full compatibility with major crates (Axum, Reqwest, etc.)
- Multi-threaded runtime for parallel scrape requests
- Graceful shutdown support (`signal` feature)

**Alternatives Considered:**
| Runtime | Rejected Reason |
|---------|-----------------|
| async-std | Smaller ecosystem, no Axum support |
| smol | Lightweight but lacks ecosystem compatibility |

---

### 2.2 HTTP Server: Axum

| Aspect | Detail |
|--------|--------|
| Crate | `axum` |
| Version | 0.7.x |
| Features | default |

**Why Axum?**
- Developed by Tokio team, optimized for ecosystem
- Tower middleware compatible (timeout, compression, metrics)
- Type-safe routing and extractor pattern
- Minimal boilerplate

**Comparison:**

| Framework | Memory | Throughput | Complexity |
|-----------|--------|------------|------------|
| Axum | Low | High | Low |
| Actix-web | Low | Highest | Medium |
| Warp | Low | High | Medium |
| Rocket | Medium | Medium | Low |

> Actix-web has the highest benchmark scores, but Axum integrates naturally with Tokio ecosystem and has cleaner code.

---

### 2.3 HTTP Client: Reqwest

| Aspect | Detail |
|--------|--------|
| Crate | `reqwest` |
| Version | 0.12.x |
| Features | `json`, `rustls-tls` |

**Why Reqwest?**
- Standard async HTTP client for Tokio
- Built-in connection pooling
- Integrated JSON serialization/deserialization
- TLS support (rustls removes OpenSSL dependency)

**Key Configuration:**
```rust
Client::builder()
    .timeout(Duration::from_secs(5))
    .pool_max_idle_per_host(10)
    .build()
```

**Alternatives:**
| Client | Rejected Reason |
|--------|-----------------|
| hyper | Low-level, requires manual implementation |
| ureq | Blocking only |
| surf | Small ecosystem |

---

### 2.4 Serialization: Serde

| Aspect | Detail |
|--------|--------|
| Core | `serde` |
| JSON | `serde_json` |
| YAML | `serde_yaml` |
| Features | `derive` |

**Usage:**

| Format | Purpose |
|--------|---------|
| JSON | Jolokia API response parsing |
| YAML | Configuration file parsing |

**jmx_exporter Compatibility:**
- jmx_exporter uses YAML configuration files
- Same format support enables easy migration

---

### 2.5 Pattern Matching: Regex

| Aspect | Detail |
|--------|--------|
| Crate | `regex` |
| Version | 1.x |

**Why Regex?**
- jmx_exporter's `pattern` syntax is based on Java regex
- Rust regex crate has high PCRE compatibility
- Capture group (`$1`, `$2`) support for dynamic metric names

**jmx_exporter Pattern Example:**
```yaml
# jmx_exporter config
- pattern: "java.lang<type=Memory><HeapMemoryUsage>(\\w+)"
  name: "jvm_memory_heap_$1_bytes"
```

**Caveats:**
- Not 100% compatible with Java regex
- Java-specific syntax like `\p{javaLowerCase}` not supported
- Most real-world patterns are compatible

---

### 2.6 Logging: tracing

| Aspect | Detail |
|--------|--------|
| Core | `tracing` |
| Subscriber | `tracing-subscriber` |
| Features | `env-filter`, `fmt` |

**Why tracing?**
- Structured logging support
- Span-based context propagation
- Perfect integration with Tokio/Axum
- Runtime log level changes

**Log Levels:**
```rust
// Controlled via environment variable
RUST_LOG=rjmx_exporter=debug,tower_http=info
```

---

### 2.7 Error Handling

| Aspect | Crate | Purpose |
|--------|-------|---------|
| Library errors | `thiserror` | Custom error type definitions |
| Application errors | `anyhow` | Error propagation and context |

**Pattern:**
```rust
// Library code - specific error types
#[derive(Debug, thiserror::Error)]
pub enum CollectorError {
    #[error("Jolokia request failed: {0}")]
    RequestFailed(#[from] reqwest::Error),

    #[error("Invalid MBean response")]
    InvalidResponse,
}

// Application code - error propagation
fn main() -> anyhow::Result<()> {
    // ...
}
```

---

### 2.8 CLI: clap

| Aspect | Detail |
|--------|--------|
| Crate | `clap` |
| Version | 4.x |
| Features | `derive` |

**Planned CLI:**
```bash
rjmx-exporter [OPTIONS]

Options:
  -c, --config <FILE>    Config file path [default: config.yaml]
  -p, --port <PORT>      Server port [default: 9090]
      --dry-run          Validate config without starting server
      --validate         Check config compatibility
  -v, --verbose          Increase logging verbosity
  -h, --help             Print help
  -V, --version          Print version
```

---

## 3. Development Dependencies

| Crate | Purpose |
|-------|---------|
| `tokio-test` | Async test utilities |
| `wiremock` | HTTP mocking for tests |
| `assert_cmd` | CLI integration tests |
| `criterion` | Benchmarking |
| `insta` | Snapshot testing |

---

## 4. Cargo.toml Preview

```toml
[package]
name = "rjmx-exporter"
version = "0.1.0"
edition = "2021"
rust-version = "1.75"

[dependencies]
# Async runtime
tokio = { version = "1", features = ["rt-multi-thread", "macros", "net", "time", "signal"] }

# HTTP
axum = "0.7"
reqwest = { version = "0.12", default-features = false, features = ["json", "rustls-tls"] }
tower = "0.4"
tower-http = { version = "0.5", features = ["timeout", "trace"] }

# Serialization
serde = { version = "1", features = ["derive"] }
serde_json = "1"
serde_yaml = "0.9"

# Pattern matching
regex = "1"

# Logging
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter", "fmt"] }

# Error handling
thiserror = "1"
anyhow = "1"

# CLI
clap = { version = "4", features = ["derive"] }

[dev-dependencies]
tokio-test = "0.4"
wiremock = "0.6"
assert_cmd = "2"
criterion = "0.5"
insta = { version = "1", features = ["yaml"] }

[[bench]]
name = "transform"
harness = false
```

---

## 5. jmx_exporter Compatibility Matrix

### 5.1 Configuration Syntax

| jmx_exporter Option | rJMX-Exporter | Notes |
|---------------------|---------------|-------|
| `rules[].pattern` | Supported | Java regex → Rust regex |
| `rules[].name` | Supported | `$1`, `$2` capture groups |
| `rules[].type` | Supported | `gauge`, `counter`, `untyped` |
| `rules[].labels` | Supported | Static + dynamic labels |
| `rules[].help` | Supported | HELP line in output |
| `lowercaseOutputName` | Supported | |
| `lowercaseOutputLabelNames` | Supported | |
| `whitelistObjectNames` | Supported | MBean filter |
| `blacklistObjectNames` | Supported | MBean exclusion |
| `rules[].value` | Planned (P2) | Custom value expression |
| `rules[].valueFactor` | Planned (P2) | Value multiplier |
| `hostPort` | Not supported | Use `jolokia.url` |
| `jmxUrl` | Not supported | Jolokia only |
| `ssl` / `sslConfig` | Not supported | Use HTTPS in URL |

### 5.2 Regex Compatibility

| Pattern Feature | Java | Rust | Status |
|-----------------|------|------|--------|
| Basic regex | Yes | Yes | Compatible |
| Capture groups | `$1` | `$1` | Compatible |
| Named groups | `(?<name>)` | `(?P<name>)` | Syntax differs |
| Unicode classes | `\p{Lower}` | `\p{Ll}` | Syntax differs |
| Possessive quantifiers | `++` | N/A | Not supported |
| Atomic groups | `(?>)` | N/A | Not supported |

> Most jmx_exporter patterns are compatible. Only watch out for complex Java-specific syntax.

### 5.3 Output Format

Prometheus exposition format output is identical:

```
# HELP jvm_memory_heap_used_bytes JVM heap memory used
# TYPE jvm_memory_heap_used_bytes gauge
jvm_memory_heap_used_bytes{area="heap"} 1.234567e+08
```

---

## 6. Performance Considerations

### 6.1 Memory Optimization

| Strategy | Implementation |
|----------|----------------|
| Zero-copy parsing | `serde_json` with borrowed strings |
| Pre-compiled regex | `lazy_static` or `OnceCell` |
| Connection reuse | Reqwest connection pool |
| String interning | Consider `compact_str` for labels |

### 6.2 Latency Optimization

| Strategy | Implementation |
|----------|----------------|
| Parallel collection | `tokio::spawn` per target |
| Request timeout | 5s default, configurable |
| Response streaming | Process JSON incrementally |

### 6.3 Binary Size

```bash
# Release build with optimizations
cargo build --release

# Further optimization
[profile.release]
lto = true
codegen-units = 1
strip = true
```

Expected binary size: **~5MB** (vs ~50MB for JVM)

---

## 7. Security Considerations

| Concern | Mitigation |
|---------|------------|
| TLS | rustls (no OpenSSL dependency) |
| Input validation | Strict config schema validation |
| DoS protection | Request timeout, rate limiting |
| Secrets | Environment variable support for auth |

---

## 8. References

### Rust Crates
- [Tokio](https://tokio.rs/) - Async runtime
- [Axum](https://docs.rs/axum) - Web framework
- [Reqwest](https://docs.rs/reqwest) - HTTP client
- [Serde](https://serde.rs/) - Serialization

### Compatibility Sources
- [jmx_exporter Config](https://github.com/prometheus/jmx_exporter#configuration)
- [Jolokia Protocol](https://jolokia.org/reference/html/protocol.html)
- [Prometheus Exposition Format](https://prometheus.io/docs/instrumenting/exposition_formats/)
