# CLI Reference

Complete command-line interface documentation for rJMX-Exporter.

## Synopsis

```
rjmx-exporter [OPTIONS]
```

## Options

| Option | Short | Description | Default |
|--------|-------|-------------|---------|
| `--config <FILE>` | `-c` | Configuration file path | `config.yaml` |
| `--port <PORT>` | `-p` | Override server port | From config |
| `--log-level <LEVEL>` | `-l` | Log level | `info` |
| `--validate` | | Validate configuration and exit | |
| `--dry-run` | | Test config, show parsed rules | |
| `--output-format <FMT>` | | Validation output format | `text` |
| `--startup-time` | | Display startup time | |
| `--help` | `-h` | Print help | |
| `--version` | `-V` | Print version | |

## Log Levels

- `trace` - Most verbose, includes all internal details
- `debug` - Debugging information
- `info` - Normal operation messages
- `warn` - Warnings that don't prevent operation
- `error` - Errors only

## Examples

### Basic Usage

```bash
# Run with default config (config.yaml)
./rjmx-exporter

# Run with custom config
./rjmx-exporter -c /etc/rjmx/config.yaml

# Run with custom port
./rjmx-exporter -c config.yaml -p 8080
```

### Configuration Validation

```bash
# Validate config syntax
./rjmx-exporter --validate -c config.yaml

# Dry run - shows parsed rules without starting server
./rjmx-exporter --dry-run -c config.yaml

# Validation with JSON output
./rjmx-exporter --validate --output-format json -c config.yaml
```

### Debugging

```bash
# Debug logging
./rjmx-exporter -c config.yaml -l debug

# Trace logging (very verbose)
./rjmx-exporter -c config.yaml -l trace

# Measure startup time
./rjmx-exporter --startup-time -c config.yaml
```

## Environment Variables

| Variable | Description | Example |
|----------|-------------|---------|
| `RJMX_PORT` | Override server port | `RJMX_PORT=8080` |
| `RJMX_LOG_LEVEL` | Set log level | `RJMX_LOG_LEVEL=debug` |
| `RUST_LOG` | Rust logging filter | `RUST_LOG=rjmx_exporter=debug` |

### Environment Variable Priority

1. Command-line flags (highest)
2. Environment variables
3. Config file
4. Default values (lowest)

## Exit Codes

| Code | Meaning |
|------|---------|
| `0` | Success |
| `1` | Configuration error |
| `2` | Connection error |
| `3` | Runtime error |

## Docker Usage

```bash
# Basic run
docker run -p 9090:9090 -v ./config.yaml:/config.yaml:ro rjmx-exporter

# With environment variables
docker run -p 9090:9090 \
  -e RJMX_LOG_LEVEL=debug \
  -v ./config.yaml:/config.yaml:ro \
  rjmx-exporter

# Validate config
docker run --rm -v ./config.yaml:/config.yaml:ro \
  rjmx-exporter --validate
```
