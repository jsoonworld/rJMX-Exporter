# CLAUDE.md - rJMX-Exporter

This file provides project context for Claude Code agents.

---

## Project Summary

**rJMX-Exporter** is a high-performance JMX Metric Exporter written in Rust, replacing the Java-based `jmx_exporter`.

### Core Objectives

| Metric | Target |
|--------|--------|
| Memory Usage | < 10MB |
| JVM Required | No |
| Target App Impact | 0% (sidecar) |
| Startup Time | < 100ms |
| Scrape Latency | < 10ms |

### Architecture

```
Java App (Jolokia) --HTTP/JSON--> rJMX-Exporter (Rust) --/metrics--> Prometheus
```

---

## Contribution Guidelines

### Language Policy

**English Only** - This is an open-source project. All contributions must be in English:

- Commit messages
- Pull request titles and descriptions
- Code comments
- Documentation
- Issue reports and discussions

### Git Workflow

```
main (protected, production-ready)
  └── develop (integration branch)
        └── feature/* (feature branches)
        └── fix/* (bug fix branches)
        └── docs/* (documentation branches)
```

### Branch Strategy

| Branch | Purpose | Merge Target |
|--------|---------|--------------|
| `main` | Production-ready code | - |
| `develop` | Integration branch | `main` (release) |
| `feature/*` | New features | `develop` |
| `fix/*` | Bug fixes | `develop` |
| `docs/*` | Documentation | `develop` |

### Pull Request Rules

1. **All PRs must target `develop` branch** (not `main`)
2. PRs to `main` are only for releases from `develop`
3. Require code review before merging
4. All CI checks must pass

### Commit Message Format

```
type: short description in English

- Detail 1
- Detail 2

Co-Authored-By: Name <email>
```

Types: `feat`, `fix`, `docs`, `refactor`, `test`, `chore`

---

## Tech Stack

| Category | Technology |
|----------|------------|
| Language | Rust 2021 Edition |
| Async Runtime | Tokio |
| HTTP Server | Axum |
| HTTP Client | Reqwest |
| Serialization | Serde (YAML, JSON) |
| Logging | tracing |
| Error Handling | thiserror / anyhow |

---

## Development Commands

```bash
# Build
cargo build
cargo build --release

# Test
cargo test
cargo test -- --nocapture

# Lint
cargo clippy
cargo clippy -- -D warnings

# Format
cargo fmt
cargo fmt -- --check

# Documentation
cargo doc --open
```

---

## Directory Structure

```
rJMX-Exporter/
├── Cargo.toml
├── CLAUDE.md              # This file
├── README.md
├── .claude/
│   └── skills/            # Claude Code skills
├── docs/
│   └── 1-PAGER.md
├── src/
│   ├── main.rs
│   ├── lib.rs
│   ├── config.rs
│   ├── error.rs
│   ├── collector/
│   ├── transformer/
│   └── server/
├── tests/
└── examples/
```

---

## Code Quality Standards

### Absolutely Forbidden

1. No `panic!()` in production code
2. No direct `unwrap()` calls
3. Minimize unsafe blocks
4. No blocking I/O in async context

### Required Practices

1. Write tests first (TDD)
2. Return `Result<T, Error>`
3. Use `tracing` for logging
4. Document all pub items

---

## Configuration Example

```yaml
targets:
  - url: "http://localhost:8778/jolokia"
    name: "my-java-app"

server:
  port: 9090
  path: "/metrics"

rules:
  - pattern: "java.lang<type=Memory><HeapMemoryUsage>(\\w+)"
    name: "jvm_memory_heap_$1_bytes"
    type: gauge
```

---

## References

- [Jolokia](https://jolokia.org/) - JMX to HTTP/JSON
- [jmx_exporter](https://github.com/prometheus/jmx_exporter) - Original Java version
- [Tokio](https://tokio.rs/) - Async runtime
- [Axum](https://github.com/tokio-rs/axum) - HTTP framework
