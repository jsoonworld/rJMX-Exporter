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

## Agent Cognitive Architecture

### Available Patterns

| Pattern | Use Case | Skill |
|---------|----------|-------|
| **ReAct** | Unpredictable problem solving | `/react` |
| **Plan-and-Execute** | Complex feature implementation | `/plan` |
| **Reflection** | Code quality review | `/reflect` |

### Pattern Selection Criteria

```
Is the workflow predictable?
├── Yes → Sequential execution
└── No → Dynamic orchestration
    ├── Speed matters → ReAct
    └── Quality matters → Add Reflection
```

---

## Skills Directory

| Skill | Description |
|-------|-------------|
| `/test` | Run tests |
| `/lint` | Code quality check (fmt + clippy) |
| `/build` | Build project |
| `/check` | Full quality check |
| `/doc` | Generate documentation |
| `/commit` | Create commit |
| `/review` | Code review |
| `/plan` | Create execution plan |
| `/react` | Execute ReAct pattern |
| `/reflect` | Self-review |
| `/debug` | Debug issues |
| `/implement` | Implement features |

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

```text
rJMX-Exporter/
├── Cargo.toml
├── CLAUDE.md              # This file
├── README.md
├── .claude/
│   ├── settings.local.json
│   ├── system_prompt_additions.md
│   └── skills/
│       ├── test.md
│       ├── lint.md
│       ├── build.md
│       ├── check.md
│       ├── doc.md
│       ├── commit.md
│       ├── review.md
│       ├── plan.md
│       ├── react.md
│       ├── reflect.md
│       ├── debug.md
│       └── implement.md
├── docs/
│   └── 1-PAGER.md
├── src/
│   ├── main.rs
│   ├── lib.rs
│   ├── config.rs
│   ├── error.rs
│   ├── collector/
│   │   ├── mod.rs
│   │   ├── client.rs         # (planned)
│   │   └── parser.rs         # (planned)
│   ├── transformer/
│   │   ├── mod.rs
│   │   └── rules.rs          # (planned)
│   └── server/
│       ├── mod.rs
│       └── handlers.rs
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

## State Management

For long-running tasks, use:

- **progress.txt**: Free-form progress notes
- **tasks.json**: Structured task list
- **git**: Commit all changes

---

## References

- [Jolokia](https://jolokia.org/) - JMX to HTTP/JSON
- [jmx_exporter](https://github.com/prometheus/jmx_exporter) - Original Java version
- [Tokio](https://tokio.rs/) - Async runtime
- [Axum](https://github.com/tokio-rs/axum) - HTTP framework

---

## Current Status

### Status: Phase 3 Transform Engine implemented (as of 2026-02-02)

### Completed

- [x] Phase 1: Create Cargo.toml and basic project structure
- [x] Phase 1: Implement Tokio + Axum basic server
- [x] Phase 1: Set up Jolokia test environment (Docker)
- [x] Phase 2: Implement Jolokia HTTP client
- [x] Phase 2: Parse Jolokia JSON responses
- [x] Phase 2: Define MBean data structures
- [x] Phase 3: Rule-based MBean pattern matching
- [x] Phase 3: Prometheus exposition format output

### Next Steps (Phase 4)

1. [ ] Benchmark resource usage vs Java version
2. [ ] Integration tests
