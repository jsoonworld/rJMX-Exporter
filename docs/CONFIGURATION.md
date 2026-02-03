# Configuration Reference

Complete configuration guide for rJMX-Exporter.

## Basic Configuration

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

## Full Configuration

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

## Configuration Options

### Jolokia Section

| Option | Required | Default | Description |
|--------|----------|---------|-------------|
| `url` | Yes | - | Jolokia endpoint URL |
| `username` | No | - | Basic auth username |
| `password` | No | - | Basic auth password |
| `timeout_ms` | No | `5000` | Request timeout in milliseconds |

### Server Section

| Option | Required | Default | Description |
|--------|----------|---------|-------------|
| `port` | No | `9090` | HTTP server port |
| `path` | No | `/metrics` | Metrics endpoint path |
| `bind_address` | No | `0.0.0.0` | Bind address |

### Global Options

| Option | Default | Description |
|--------|---------|-------------|
| `lowercaseOutputName` | `false` | Lowercase metric names |
| `lowercaseOutputLabelNames` | `false` | Lowercase label names |
| `whitelistObjectNames` | `[]` | MBean patterns to include |
| `blacklistObjectNames` | `[]` | MBean patterns to exclude |

### Rule Options

| Option | Required | Description |
|--------|----------|-------------|
| `pattern` | Yes | Regex pattern to match MBean names |
| `name` | Yes | Prometheus metric name (`$1`, `$2` for capture groups) |
| `type` | Yes | Metric type: `gauge`, `counter`, or `untyped` |
| `help` | No | Help text for the metric |
| `labels` | No | Static or dynamic labels |
| `valueFactor` | No | Multiply metric value (e.g., `0.001` for ms to s) |

## Pattern Matching

rJMX-Exporter uses the same pattern format as jmx_exporter:

```
domain<key1=value1,key2=value2><attribute>subattribute
```

### Examples

```yaml
# Match heap memory usage
- pattern: 'java.lang<type=Memory><HeapMemoryUsage>(\w+)'
  name: "jvm_memory_heap_$1_bytes"

# Match GC collectors with name label
- pattern: 'java.lang<type=GarbageCollector,name=([^>]+)><CollectionCount>'
  name: "jvm_gc_collection_count"
  labels:
    gc: "$1"

# Match thread counts
- pattern: 'java.lang<type=Threading><(\w+)>'
  name: "jvm_threads_$1"
```

## Environment Variable Overrides

| Variable | Description |
|----------|-------------|
| `RJMX_PORT` | Override server port |
| `RJMX_LOG_LEVEL` | Log level (trace, debug, info, warn, error) |
| `RUST_LOG` | Rust logging filter |
| `RJMX_TLS_ENABLED` | Enable TLS (true/false) |
| `RJMX_TLS_CERT_FILE` | Path to TLS certificate file (PEM format) |
| `RJMX_TLS_KEY_FILE` | Path to TLS private key file (PEM format) |

## TLS Configuration

rJMX-Exporter supports HTTPS for secure metrics exposure.

### CLI Options

```bash
rjmx-exporter --config config.yaml \
  --tls-enabled \
  --tls-cert-file /path/to/cert.pem \
  --tls-key-file /path/to/key.pem
```

### Configuration File

```yaml
server:
  port: 9090
  path: "/metrics"
  tls:
    enabled: true
    cert_file: "/path/to/cert.pem"
    key_file: "/path/to/key.pem"
```

### Requirements

- Both `cert_file` and `key_file` are required when TLS is enabled
- Certificate and key must be in PEM format
- The certificate chain should include intermediate certificates if needed
