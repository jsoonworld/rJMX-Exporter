# rJMX-Exporter Helm Chart

A Helm chart for deploying rJMX-Exporter - a high-performance JMX Metric Exporter written in Rust.

## Introduction

This chart deploys rJMX-Exporter on a Kubernetes cluster using the Helm package manager. rJMX-Exporter is designed as a lightweight, efficient replacement for the Java-based jmx_exporter, featuring:

- Memory usage < 10MB
- No JVM required
- Startup time < 100ms
- Scrape latency < 10ms

## Prerequisites

- Kubernetes 1.19+
- Helm 3.0+
- Java application(s) with Jolokia agent enabled

## Installation

### Add the repository (if published)

```bash
helm repo add rjmx-exporter https://jsoonworld.github.io/rJMX-Exporter
helm repo update
```

### Install from local chart

```bash
# Clone the repository
git clone https://github.com/jsoonworld/rJMX-Exporter.git
cd rJMX-Exporter

# Install the chart
helm install rjmx-exporter ./charts/rjmx-exporter
```

### Install with custom values

```bash
helm install rjmx-exporter ./charts/rjmx-exporter -f my-values.yaml
```

### Install in a specific namespace

```bash
helm install rjmx-exporter ./charts/rjmx-exporter --namespace monitoring --create-namespace
```

## Uninstallation

```bash
helm uninstall rjmx-exporter
```

## Configuration

The following table lists the configurable parameters and their default values.

### General

| Parameter | Description | Default |
|-----------|-------------|---------|
| `replicaCount` | Number of replicas | `1` |
| `nameOverride` | Override the chart name | `""` |
| `fullnameOverride` | Override the full name | `""` |

### Image

| Parameter | Description | Default |
|-----------|-------------|---------|
| `image.repository` | Image repository | `rjmx-exporter` |
| `image.tag` | Image tag | `""` (uses appVersion) |
| `image.pullPolicy` | Image pull policy | `IfNotPresent` |
| `imagePullSecrets` | Image pull secrets | `[]` |

### Service Account

| Parameter | Description | Default |
|-----------|-------------|---------|
| `serviceAccount.create` | Create service account | `true` |
| `serviceAccount.annotations` | Service account annotations | `{}` |
| `serviceAccount.name` | Service account name | `""` |

### Service

| Parameter | Description | Default |
|-----------|-------------|---------|
| `service.type` | Service type | `ClusterIP` |
| `service.port` | Service port | `9090` |
| `service.targetPort` | Target port | `9090` |
| `service.annotations` | Service annotations | `{}` |

### Resources

| Parameter | Description | Default |
|-----------|-------------|---------|
| `resources.limits.cpu` | CPU limit | `100m` |
| `resources.limits.memory` | Memory limit | `50Mi` |
| `resources.requests.cpu` | CPU request | `50m` |
| `resources.requests.memory` | Memory request | `20Mi` |

### Security Context

| Parameter | Description | Default |
|-----------|-------------|---------|
| `podSecurityContext.runAsNonRoot` | Run as non-root | `true` |
| `podSecurityContext.runAsUser` | User ID | `1000` |
| `podSecurityContext.runAsGroup` | Group ID | `1000` |
| `podSecurityContext.fsGroup` | FS Group ID | `1000` |
| `securityContext.allowPrivilegeEscalation` | Allow privilege escalation | `false` |
| `securityContext.readOnlyRootFilesystem` | Read-only root filesystem | `true` |

### rJMX-Exporter Configuration

| Parameter | Description | Default |
|-----------|-------------|---------|
| `config.server.port` | Server port | `9090` |
| `config.server.path` | Metrics path | `/metrics` |
| `config.targets` | Jolokia targets | See values.yaml |
| `config.rules` | Metric transformation rules | See values.yaml |

### Jolokia Authentication

| Parameter | Description | Default |
|-----------|-------------|---------|
| `jolokiaAuth.enabled` | Enable authentication | `false` |
| `jolokiaAuth.username` | Username | `""` |
| `jolokiaAuth.password` | Password | `""` |

### Probes

| Parameter | Description | Default |
|-----------|-------------|---------|
| `probes.liveness.enabled` | Enable liveness probe | `true` |
| `probes.liveness.path` | Liveness probe path | `/health` |
| `probes.readiness.enabled` | Enable readiness probe | `true` |
| `probes.readiness.path` | Readiness probe path | `/health` |

### ServiceMonitor (Prometheus Operator)

| Parameter | Description | Default |
|-----------|-------------|---------|
| `serviceMonitor.enabled` | Enable ServiceMonitor | `false` |
| `serviceMonitor.namespace` | ServiceMonitor namespace | `""` |
| `serviceMonitor.interval` | Scrape interval | `30s` |
| `serviceMonitor.scrapeTimeout` | Scrape timeout | `10s` |
| `serviceMonitor.labels` | Additional labels | `{}` |

## Usage Examples

### Basic Installation

```bash
helm install rjmx-exporter ./charts/rjmx-exporter
```

### Custom Target Configuration

Create a `custom-values.yaml`:

```yaml
config:
  targets:
    - url: "http://my-java-app:8778/jolokia"
      name: "my-java-app"
    - url: "http://another-app:8778/jolokia"
      name: "another-app"
```

Install with custom values:

```bash
helm install rjmx-exporter ./charts/rjmx-exporter -f custom-values.yaml
```

### With Jolokia Authentication

```yaml
jolokiaAuth:
  enabled: true
  username: "admin"
  password: "secret"
```

### With Prometheus Operator

```yaml
serviceMonitor:
  enabled: true
  interval: 15s
  labels:
    release: prometheus
```

### Custom Metric Rules

```yaml
config:
  rules:
    - pattern: "java.lang<type=Memory><HeapMemoryUsage>(\\w+)"
      name: "jvm_memory_heap_$1_bytes"
      type: gauge
    - pattern: "com.example<type=MyBean><>(\\w+)"
      name: "myapp_$1"
      type: gauge
      labels:
        app: "myapp"
```

### High Availability Setup

```yaml
replicaCount: 3

affinity:
  podAntiAffinity:
    preferredDuringSchedulingIgnoredDuringExecution:
      - weight: 100
        podAffinityTerm:
          labelSelector:
            matchLabels:
              app.kubernetes.io/name: rjmx-exporter
          topologyKey: kubernetes.io/hostname
```

## Accessing Metrics

After installation, you can access the metrics:

```bash
# Port forward to local machine
kubectl port-forward svc/rjmx-exporter 9090:9090

# Access metrics
curl http://localhost:9090/metrics

# Check health
curl http://localhost:9090/health
```

## Troubleshooting

### Check pod status

```bash
kubectl get pods -l app.kubernetes.io/name=rjmx-exporter
```

### View logs

```bash
kubectl logs -l app.kubernetes.io/name=rjmx-exporter
```

### Check configuration

```bash
kubectl get configmap -l app.kubernetes.io/name=rjmx-exporter -o yaml
```

## Contributing

Please see the [main repository](https://github.com/jsoonworld/rJMX-Exporter) for contribution guidelines.

## License

This project is licensed under the MIT License.
