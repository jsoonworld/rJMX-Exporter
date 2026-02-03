# Migration from jmx_exporter

This guide helps you migrate from the Java-based jmx_exporter to rJMX-Exporter.

## Overview

rJMX-Exporter is designed as a drop-in replacement for jmx_exporter. Your existing rule configurations work with minimal changes.

## Step-by-Step Migration

### Step 1: Add Jolokia to Your JVM

Unlike jmx_exporter (which can connect directly via RMI), rJMX-Exporter requires Jolokia for JMX access.

```bash
# Download Jolokia agent
wget https://repo1.maven.org/maven2/org/jolokia/jolokia-jvm/1.7.2/jolokia-jvm-1.7.2.jar

# Add to your Java startup command
java -javaagent:jolokia-jvm-1.7.2.jar=port=8778,host=0.0.0.0 -jar your-app.jar
```

### Step 2: Update Your Config

Add the Jolokia endpoint to your existing config:

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

### Step 4: Update Prometheus Config

Point Prometheus to the new exporter:

```yaml
scrape_configs:
  - job_name: "jvm"
    static_configs:
      - targets: ["localhost:9090"]  # rJMX-Exporter port
```

## Compatibility Matrix

| jmx_exporter Option | rJMX-Exporter | Notes |
|---------------------|---------------|-------|
| `rules[].pattern` | Supported | Full regex with capture groups |
| `rules[].name` | Supported | `$1`, `$2` substitution |
| `rules[].type` | Supported | gauge, counter, untyped |
| `rules[].labels` | Supported | Static and dynamic labels |
| `rules[].help` | Supported | |
| `rules[].valueFactor` | Supported | |
| `whitelistObjectNames` | Supported | Glob patterns |
| `blacklistObjectNames` | Supported | Glob patterns |
| `lowercaseOutputName` | Supported | |
| `lowercaseOutputLabelNames` | Supported | |
| `hostPort` | **Not supported** | Use `jolokia.url` instead |
| `jmxUrl` | **Not supported** | Jolokia only, no direct RMI |
| `ssl` | **Not supported** | Use HTTPS in Jolokia URL |
| `username/password` (RMI) | **Not supported** | Use Jolokia basic auth |

## Key Differences

### Connection Method

| jmx_exporter | rJMX-Exporter |
|--------------|---------------|
| Direct RMI connection | HTTP via Jolokia |
| `hostPort: localhost:9999` | `jolokia.url: http://localhost:8778/jolokia` |

### Deployment Model

| jmx_exporter | rJMX-Exporter |
|--------------|---------------|
| javaagent (in-process) or standalone JVM | Native binary sidecar |
| Shares JVM heap | Complete isolation |

## Troubleshooting

### Jolokia Not Responding

```bash
# Test Jolokia endpoint
curl http://localhost:8778/jolokia/version
```

Expected response:
```json
{"value":{"agent":"1.7.2","protocol":"7.2"},"status":200}
```

### No Metrics Appearing

1. Check if your rules match the MBean names:
   ```bash
   ./rjmx-exporter --dry-run -c config.yaml
   ```

2. List available MBeans:
   ```bash
   curl 'http://localhost:8778/jolokia/list' | jq '.value | keys'
   ```

### Different Metric Values

rJMX-Exporter fetches MBean values at scrape time, same as jmx_exporter standalone mode. If you were using javaagent mode, values might differ slightly due to timing.
