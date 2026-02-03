# rJMX-Exporter

[![CI](https://github.com/jsoonworld/rJMX-Exporter/actions/workflows/ci.yml/badge.svg)](https://github.com/jsoonworld/rJMX-Exporter/actions/workflows/ci.yml)
[![License](https://img.shields.io/badge/license-MIT%2FApache--2.0-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.75%2B-orange.svg)](https://www.rust-lang.org/)

**A high-performance JMX metrics exporter for Prometheus, written in Rust.**

rJMX-Exporter collects JMX metrics from JVM applications via [Jolokia](https://jolokia.org/) and exports them in Prometheus format — without requiring a JVM.

## Why rJMX-Exporter?

The official [jmx_exporter](https://github.com/prometheus/jmx_exporter) is widely used for monitoring JVM applications (Kafka, Cassandra, Spring Boot, etc.), but it has architectural limitations:

| Mode | How It Works | Problems |
|------|--------------|----------|
| **javaagent** | Runs inside target JVM | Shares heap & GC with your app. Can cause OOM under memory pressure. Adds latency during GC pauses. |
| **standalone** | Separate JVM process | Requires JVM installation. ~50MB+ memory overhead per instance. |

**rJMX-Exporter solves these problems:**

- **Native binary** — No JVM required, runs anywhere
- **Complete isolation** — Zero impact on your application's heap/GC
- **Minimal footprint** — <10MB memory usage
- **Fast startup** — <100ms cold start
- **Drop-in replacement** — Compatible with jmx_exporter YAML rules

### Performance Comparison

| Metric | jmx_exporter (Java) | rJMX-Exporter (Rust) |
|--------|---------------------|----------------------|
| Memory Usage | ~50MB+ | **<10MB** |
| Startup Time | 2-5 seconds | **<100ms** |
| JVM Required | Yes | **No** |
| Impact on Target App | Shares GC/Heap (agent) | **None** |
| Binary Size | N/A (needs JVM) | **~5MB** |

## Features

- **Prometheus `/metrics` endpoint** with standard text exposition format
- **Rule-based transformation** with regex pattern matching
- **jmx_exporter YAML compatibility** — migrate existing configs easily
- **Dynamic label extraction** from MBean attributes using capture groups
- **Whitelist/blacklist filtering** for MBean selection
- **Health endpoint** (`/health`) for container orchestration
- **Docker support** with multi-stage builds and Docker Compose
- **Comprehensive CLI** with validation, dry-run, and debug modes

## Quick Start

### Option 1: Docker Compose (Recommended)

The fastest way to see rJMX-Exporter in action:

```bash
# Clone the repository
git clone https://github.com/jsoonworld/rJMX-Exporter.git
cd rJMX-Exporter

# Start the full stack (Java app + rJMX-Exporter + Prometheus)
docker compose up -d

# View metrics
curl http://localhost:9090/metrics

# Access Prometheus UI
open http://localhost:9091
```

**Available endpoints:**
- Java App (Jolokia): http://localhost:8778/jolokia
- rJMX-Exporter: http://localhost:9090/metrics
- Prometheus: http://localhost:9091

### Option 2: Build from Source

```bash
# Build release binary
cargo build --release

# Copy example config
cp config.example.yaml config.yaml

# Edit config to point to your Jolokia endpoint
vim config.yaml

# Run
./target/release/rjmx-exporter -c config.yaml
```

### Option 3: With Existing Java Application

1. **Add Jolokia to your JVM application:**

```bash
# Download Jolokia agent
wget https://repo1.maven.org/maven2/org/jolokia/jolokia-jvm/1.7.2/jolokia-jvm-1.7.2.jar

# Add to your Java startup command
java -javaagent:jolokia-jvm-1.7.2.jar=port=8778,host=0.0.0.0 -jar your-app.jar
```

2. **Configure rJMX-Exporter:**

```yaml
# config.yaml
jolokia:
  url: "http://localhost:8778/jolokia"
  timeout_ms: 5000

server:
  port: 9090
  path: "/metrics"

rules:
  - pattern: 'java.lang<type=Memory><HeapMemoryUsage>(\w+)'
    name: "jvm_memory_heap_$1_bytes"
    type: gauge
```

3. **Run rJMX-Exporter:**

```bash
./rjmx-exporter -c config.yaml
```

4. **Configure Prometheus:**

```yaml
# prometheus.yml
scrape_configs:
  - job_name: "jvm"
    static_configs:
      - targets: ["localhost:9090"]
```

## Configuration

### Basic Configuration

```yaml
jolokia:
  url: "http://localhost:8778/jolokia"
  timeout_ms: 5000

server:
  port: 9090
  path: "/metrics"

rules:
  - pattern: 'java.lang<type=Memory><HeapMemoryUsage>(\w+)'
    name: "jvm_memory_heap_$1_bytes"
    type: gauge
    help: "JVM heap memory usage"
```

### Full Configuration

```yaml
# Jolokia endpoint
jolokia:
  url: "http://localhost:8778/jolokia"
  username: "jolokia"        # Optional: basic auth
  password: "secret"         # Optional: basic auth
  timeout_ms: 5000           # Request timeout

# HTTP server
server:
  port: 9090
  path: "/metrics"
  bind_address: "0.0.0.0"    # Or "127.0.0.1" for local only

# jmx_exporter compatible options
lowercaseOutputName: true
lowercaseOutputLabelNames: true

# MBean filtering (glob patterns)
whitelistObjectNames:
  - "java.lang:*"
  - "java.nio:*"

blacklistObjectNames:
  - "java.lang:type=MemoryPool,*"

# Transformation rules
rules:
  # Memory metrics
  - pattern: 'java.lang<type=Memory><HeapMemoryUsage>(\w+)'
    name: "jvm_memory_heap_$1_bytes"
    type: gauge
    help: "JVM heap memory usage"

  # GC metrics with dynamic labels
  - pattern: 'java.lang<type=GarbageCollector,name=([^>]+)><CollectionCount>'
    name: "jvm_gc_collection_count"
    type: counter
    help: "GC collection count"
    labels:
      gc: "$1"                # Capture group becomes label value

  # Thread metrics
  - pattern: 'java.lang<type=Threading><(\w+)>'
    name: "jvm_threads_$1"
    type: gauge
```

### Rule Options

| Option | Required | Description |
|--------|----------|-------------|
| `pattern` | Yes | Regex pattern to match MBean names (jmx_exporter format) |
| `name` | Yes | Prometheus metric name (`$1`, `$2` for capture groups) |
| `type` | Yes | Metric type: `gauge`, `counter`, or `untyped` |
| `help` | No | Help text for the metric |
| `labels` | No | Static or dynamic labels (`$1` for capture groups) |
| `valueFactor` | No | Multiply metric value (e.g., `0.001` for ms to s) |

## Migration from jmx_exporter

Migrating from jmx_exporter is straightforward — your existing rules work with minimal changes.

### Step 1: Add Jolokia to Your JVM

```bash
# Add to your Java startup
java -javaagent:jolokia-jvm.jar=port=8778,host=0.0.0.0 -jar your-app.jar
```

### Step 2: Update Your Config

```diff
+ jolokia:
+   url: "http://localhost:8778/jolokia"
+
  lowercaseOutputName: true
  rules:
    - pattern: "java.lang<type=Memory><HeapMemoryUsage>(\\w+)"
      name: "jvm_memory_heap_$1_bytes"
      type: gauge
```

### Step 3: Run rJMX-Exporter

```bash
./rjmx-exporter -c config.yaml
```

### Compatibility Matrix

| Option | Status | Notes |
|--------|--------|-------|
| `rules[].pattern` | Supported | Full regex with capture groups |
| `rules[].name` | Supported | `$1`, `$2` substitution |
| `rules[].type` | Supported | gauge, counter, untyped |
| `rules[].labels` | Supported | Static and dynamic |
| `rules[].help` | Supported | |
| `rules[].valueFactor` | Supported | |
| `whitelistObjectNames` | Supported | Glob patterns |
| `blacklistObjectNames` | Supported | Glob patterns |
| `lowercaseOutputName` | Supported | |
| `lowercaseOutputLabelNames` | Supported | |
| `hostPort` | Not supported | Use `jolokia.url` instead |
| `jmxUrl` | Not supported | Jolokia only, no direct RMI |

## Architecture

```
┌─────────────────┐      ┌─────────────────┐      ┌─────────────────┐
│    JVM App      │      │  rJMX-Exporter  │      │   Prometheus    │
│                 │      │     (Rust)      │      │                 │
│  ┌───────────┐  │      │                 │      │                 │
│  │  Jolokia  │◄─┼──────┤  Collector      │      │                 │
│  │  Agent    │  │ HTTP │       ↓         │      │                 │
│  └───────────┘  │ JSON │  Transformer    │      │                 │
│                 │      │       ↓         │      │                 │
│                 │      │  /metrics  ◄────┼──────┤  Scraper        │
└─────────────────┘      └─────────────────┘      └─────────────────┘
```

**Components:**
- **Collector**: Fetches JMX data from Jolokia via HTTP/JSON
- **Transformer**: Applies rules to convert MBeans to Prometheus metrics
- **Server**: Exposes `/metrics` endpoint for Prometheus scraping

## CLI Reference

```
rjmx-exporter [OPTIONS]

Options:
  -c, --config <FILE>      Configuration file path [default: config.yaml]
  -p, --port <PORT>        Override server port (env: RJMX_PORT)
  -l, --log-level <LEVEL>  Log level: trace, debug, info, warn, error
                           [default: info] (env: RJMX_LOG_LEVEL)
      --validate           Validate configuration and exit
      --dry-run            Test configuration and show parsed rules
      --output-format      Output format for validation: text, json, yaml
      --startup-time       Measure and display startup time
  -h, --help               Print help
  -V, --version            Print version
```

### Examples

```bash
# Run with custom config
./rjmx-exporter -c /etc/rjmx/config.yaml

# Validate configuration
./rjmx-exporter --validate -c config.yaml

# Dry run (shows parsed rules without starting server)
./rjmx-exporter --dry-run -c config.yaml

# Debug mode with verbose logging
./rjmx-exporter -c config.yaml -l debug

# Override port via environment
RJMX_PORT=8080 ./rjmx-exporter -c config.yaml
```

### Environment Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `RJMX_PORT` | Override server port | From config |
| `RJMX_LOG_LEVEL` | Log level | `info` |
| `RUST_LOG` | Rust logging filter | - |

## Development

### Prerequisites

- Rust 1.75+ (install via [rustup](https://rustup.rs/))
- Docker & Docker Compose (for integration tests)

### Build

```bash
# Debug build
cargo build

# Release build (optimized)
cargo build --release

# Check without building
cargo check
```

### Test

```bash
# Run all tests
cargo test

# Run with output
cargo test -- --nocapture

# Run specific test
cargo test test_name

# Integration tests (requires Docker)
docker compose up -d java-app
cargo test --test integration
```

### Lint

```bash
# Format code
cargo fmt

# Check formatting
cargo fmt -- --check

# Clippy lints
cargo clippy

# Strict mode (CI)
cargo clippy -- -D warnings
```

## Docker

### Build Image

```bash
docker build -t rjmx-exporter .
```

### Run Container

```bash
docker run -d \
  -p 9090:9090 \
  -v $(pwd)/config.yaml:/config.yaml:ro \
  rjmx-exporter
```

### Docker Compose

```bash
# Start full stack
docker compose up -d

# With Grafana (optional)
docker compose --profile monitoring up -d

# View logs
docker compose logs -f rjmx-exporter

# Stop all
docker compose down
```

## Endpoints

| Endpoint | Description |
|----------|-------------|
| `/` | Landing page with links |
| `/health` | Health check (JSON) |
| `/metrics` | Prometheus metrics (configurable path) |

### Health Check Response

```json
{
  "status": "healthy",
  "version": "0.1.0"
}
```

## Tech Stack

| Component | Technology |
|-----------|------------|
| Language | Rust 2021 Edition |
| Async Runtime | Tokio |
| HTTP Server | Axum |
| HTTP Client | Reqwest |
| Serialization | Serde (YAML, JSON) |
| Logging | tracing |
| CLI | clap |

## Project Status

**Current: v0.1.0 (Phase 4 Complete)**

- [x] Phase 1: Foundation — Project structure, Axum server, Docker environment
- [x] Phase 2: Collector — Jolokia HTTP client, JSON parsing, MBean structures
- [x] Phase 3: Transform Engine — Rule matching, Prometheus formatting
- [x] Phase 4: Validation — Benchmarks, integration tests

See [CHANGELOG.md](CHANGELOG.md) for version history.

## Documentation

- [Design Document (1-Pager)](docs/1-PAGER.md)
- [Technology Stack](docs/TECH-STACK.md)
- [Contributing Guide](CONTRIBUTING.md)
- [Security Policy](SECURITY.md)
- [Changelog](CHANGELOG.md)

## Contributing

Contributions are welcome! Please read [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.

### Quick Start

1. Fork the repository
2. Create a feature branch: `git checkout -b feature/my-feature`
3. Make your changes
4. Run tests: `cargo test`
5. Run lints: `cargo fmt && cargo clippy`
6. Commit: `git commit -m "feat: add my feature"`
7. Push and create a PR to `develop` branch

### Branch Strategy

| Branch | Purpose |
|--------|---------|
| `main` | Production-ready releases |
| `develop` | Integration branch |
| `feature/*` | New features |
| `fix/*` | Bug fixes |

## License

This project is dual-licensed under [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE) at your option.

## Acknowledgments

- [jmx_exporter](https://github.com/prometheus/jmx_exporter) — The original Java implementation
- [Jolokia](https://jolokia.org/) — JMX-HTTP bridge that makes this possible
- [Tokio](https://tokio.rs/) & [Axum](https://github.com/tokio-rs/axum) — Excellent async Rust ecosystem

---

**Made with Rust**
