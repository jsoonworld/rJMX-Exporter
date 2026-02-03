# rJMX-Exporter

[![CI](https://github.com/jsoonworld/rJMX-Exporter/actions/workflows/ci.yml/badge.svg)](https://github.com/jsoonworld/rJMX-Exporter/actions/workflows/ci.yml)
[![License](https://img.shields.io/badge/license-MIT%2FApache--2.0-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.75%2B-orange.svg)](https://www.rust-lang.org/)

A high-performance JMX metrics exporter for Prometheus, written in Rust. Collects JMX metrics via [Jolokia](https://jolokia.org/) without requiring a JVM.

## Installation

<details>
<summary><b>From Source (Recommended)</b></summary>

```bash
git clone https://github.com/jsoonworld/rJMX-Exporter.git
cd rJMX-Exporter
cargo build --release
# Binary at ./target/release/rjmx-exporter
```

</details>

<details>
<summary><b>Docker</b></summary>

```bash
docker pull ghcr.io/jsoonworld/rjmx-exporter:latest
# or build locally
docker build -t rjmx-exporter .
```

</details>

<details>
<summary><b>Docker Compose (Full Stack Demo)</b></summary>

```bash
git clone https://github.com/jsoonworld/rJMX-Exporter.git
cd rJMX-Exporter
docker compose up -d
# Java App: http://localhost:8778/jolokia
# Metrics:  http://localhost:9090/metrics
# Prometheus: http://localhost:9091
```

</details>

## Why rJMX-Exporter?

The official [jmx_exporter](https://github.com/prometheus/jmx_exporter) requires a JVM and either shares your app's heap (javaagent) or adds ~50MB overhead (standalone). rJMX-Exporter runs as a native binary sidecar with zero impact on your application.

| Metric | jmx_exporter | rJMX-Exporter |
|--------|--------------|---------------|
| Memory | ~50MB+ | **<10MB** |
| Startup | 2-5s | **<100ms** |
| JVM Required | Yes | **No** |
| App Impact | Shares GC/Heap | **None** |

## Limitations

Be aware of these trade-offs:

- **Requires Jolokia** - Your JVM needs the Jolokia agent for HTTP/JSON access to JMX
- **No direct RMI** - Cannot connect to JMX via RMI protocol (use Jolokia instead)
- **Newer project** - Less battle-tested than jmx_exporter in production environments

## Quick Start

**1. Add Jolokia to your JVM:**

```bash
java -javaagent:jolokia-jvm-1.7.2.jar=port=8778,host=0.0.0.0 -jar your-app.jar
```

**2. Create config.yaml:**

```yaml
jolokia:
  url: "http://localhost:8778/jolokia"

server:
  port: 9090

rules:
  - pattern: 'java.lang<type=Memory><HeapMemoryUsage>(\w+)'
    name: "jvm_memory_heap_$1_bytes"
    type: gauge
```

**3. Run:**

```bash
./rjmx-exporter -c config.yaml
curl http://localhost:9090/metrics
```

## Documentation

| Document | Description |
|----------|-------------|
| [Configuration](docs/CONFIGURATION.md) | Full config reference |
| [Migration Guide](docs/MIGRATION.md) | Migrate from jmx_exporter |
| [CLI Reference](docs/CLI.md) | Command-line options |
| [Design Doc](docs/1-PAGER.md) | Architecture and design decisions |

## Architecture

```
JVM App (Jolokia) --HTTP/JSON--> rJMX-Exporter --/metrics--> Prometheus
```

## Contributing

Contributions welcome! Please read [CONTRIBUTING.md](CONTRIBUTING.md).

```bash
# Development
cargo test          # Run tests
cargo clippy        # Lint
cargo fmt           # Format

# PRs target `develop` branch, not `main`
```

## License

Dual-licensed under [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE).

## Acknowledgments

- [jmx_exporter](https://github.com/prometheus/jmx_exporter) - The original Java implementation
- [Jolokia](https://jolokia.org/) - JMX-HTTP bridge
- [Tokio](https://tokio.rs/) & [Axum](https://github.com/tokio-rs/axum) - Async Rust ecosystem
