#!/bin/bash
# scripts/benchmark.sh
#
# Benchmark script for comparing jmx_exporter vs rJMX-Exporter
#
# Prerequisites:
#   - Docker and docker-compose
#   - curl
#   - jq (for JSON parsing)
#   - wrk (optional, for load testing)
#   - bc (for calculations)
#
# Usage:
#   ./scripts/benchmark.sh           # Run full benchmark
#   ./scripts/benchmark.sh --quick   # Quick smoke test
#   ./scripts/benchmark.sh --no-docker  # Use existing services

set -e

# Configuration
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"
RESULTS_DIR="$PROJECT_DIR/benchmark/results"
RESULTS_FILE="$RESULTS_DIR/benchmark-$(date +%Y%m%d-%H%M%S).json"

WARMUP_REQUESTS=10
TEST_REQUESTS=100
TEST_DURATION=30
CONCURRENT_CONNECTIONS=10
THREADS=4

# URLs (can be overridden by environment variables)
RJMX_URL="${RJMX_URL:-http://localhost:9090/metrics}"
JMX_URL="${JMX_URL:-http://localhost:9091/metrics}"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Parse arguments
QUICK_MODE=false
USE_DOCKER=true
for arg in "$@"; do
    case $arg in
        --quick)
            QUICK_MODE=true
            WARMUP_REQUESTS=3
            TEST_REQUESTS=20
            TEST_DURATION=5
            ;;
        --no-docker)
            USE_DOCKER=false
            ;;
    esac
done

# Helper functions
log_info() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

log_success() {
    echo -e "${GREEN}[OK]${NC} $1"
}

log_warn() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

check_command() {
    if ! command -v "$1" &> /dev/null; then
        log_warn "$1 is not installed. Some features may be limited."
        return 1
    fi
    return 0
}

wait_for_service() {
    local url=$1
    local name=$2
    local max_attempts=30
    local attempt=1

    log_info "Waiting for $name to be ready..."
    while [[ $attempt -le $max_attempts ]]; do
        if curl -sf "$url" > /dev/null 2>&1; then
            log_success "$name is ready"
            return 0
        fi
        sleep 2
        attempt=$((attempt + 1))
    done
    log_error "$name failed to start"
    return 1
}

# Latency measurement function
measure_latency() {
    local url=$1
    local name=$2
    local count=${3:-100}

    log_info "Measuring latency for $name ($count requests)..."

    local total_time=0
    local min_time=999999
    local max_time=0
    local times=()

    for i in $(seq 1 $count); do
        # Use curl to measure time in milliseconds
        local time_ms=$(curl -w "%{time_total}" -o /dev/null -s "$url" 2>/dev/null | awk '{printf "%.3f", $1 * 1000}')

        if [ -n "$time_ms" ] && [ "$time_ms" != "0.000" ]; then
            times+=("$time_ms")
            total_time=$(echo "$total_time + $time_ms" | bc)

            if (( $(echo "$time_ms < $min_time" | bc -l) )); then
                min_time=$time_ms
            fi
            if (( $(echo "$time_ms > $max_time" | bc -l) )); then
                max_time=$time_ms
            fi
        fi
    done

    local actual_count=${#times[@]}
    if [ "$actual_count" -eq 0 ]; then
        echo '{"error": "no successful requests"}'
        return
    fi

    local avg_time=$(echo "scale=3; $total_time / $actual_count" | bc)

    # Calculate P50 and P99
    IFS=$'\n' sorted=($(sort -n <<<"${times[*]}")); unset IFS
    local p50_idx=$((actual_count * 50 / 100))
    local p99_idx=$((actual_count * 99 / 100))
    local p50=${sorted[$p50_idx]}
    local p99=${sorted[$p99_idx]}

    echo "    Requests: $actual_count"
    echo "    Avg: ${avg_time}ms"
    echo "    Min: ${min_time}ms"
    echo "    Max: ${max_time}ms"
    echo "    P50: ${p50}ms"
    echo "    P99: ${p99}ms"

    # Return as JSON (on last line)
    echo "{\"avg\": $avg_time, \"min\": $min_time, \"max\": $max_time, \"p50\": $p50, \"p99\": $p99, \"count\": $actual_count}"
}

# Main script
echo "=============================================="
echo "   rJMX-Exporter Benchmark Suite"
echo "=============================================="
echo ""

# Check prerequisites
log_info "Checking prerequisites..."
check_command curl || exit 1
check_command bc || exit 1
HAS_JQ=$(check_command jq && echo "true" || echo "false")
HAS_WRK=$(check_command wrk && echo "true" || echo "false")
HAS_DOCKER=$(check_command docker && echo "true" || echo "false")

# Create results directory
mkdir -p "$RESULTS_DIR"

# Start services if using Docker
if [[ "$USE_DOCKER" == "true" && "$HAS_DOCKER" == "true" ]]; then
    log_info "Starting benchmark environment..."
    cd "$PROJECT_DIR"

    # Stop any existing containers
    docker-compose -f docker-compose.benchmark.yaml down -v 2>/dev/null || true

    # Build and start
    docker-compose -f docker-compose.benchmark.yaml up -d --build

    # Wait for services
    wait_for_service "http://localhost:8778/jolokia/version" "Java App (Jolokia)" || exit 1
    wait_for_service "$JMX_URL" "jmx_exporter" || exit 1
    wait_for_service "$RJMX_URL" "rJMX-Exporter" || exit 1
else
    log_info "Using existing services (--no-docker mode)"
    log_info "  rJMX-Exporter: $RJMX_URL"
    log_info "  jmx_exporter:  $JMX_URL"
fi

echo ""
log_info "All services are ready!"
echo ""

# Warmup phase
log_info "Warming up ($WARMUP_REQUESTS requests per endpoint)..."
for i in $(seq 1 $WARMUP_REQUESTS); do
    curl -sf "$RJMX_URL" > /dev/null 2>&1 || true
    curl -sf "$JMX_URL" > /dev/null 2>&1 || true
done
log_success "Warmup complete"

echo ""
echo "=============================================="
echo "   Memory Usage Comparison"
echo "=============================================="
echo ""

# Get memory usage from docker stats
RJMX_MEM="N/A"
JMX_MEM="N/A"
JAVA_MEM="N/A"

if [[ "$HAS_DOCKER" == "true" && "$USE_DOCKER" == "true" ]]; then
    log_info "Measuring memory usage..."
    sleep 2

    RJMX_STATS=$(docker stats --no-stream --format "{{.MemUsage}}" benchmark-rjmx-exporter 2>/dev/null || echo "N/A")
    JMX_STATS=$(docker stats --no-stream --format "{{.MemUsage}}" benchmark-jmx-exporter 2>/dev/null || echo "N/A")
    JAVA_STATS=$(docker stats --no-stream --format "{{.MemUsage}}" benchmark-java-app 2>/dev/null || echo "N/A")

    RJMX_MEM=$(echo "$RJMX_STATS" | cut -d'/' -f1 | tr -d ' ')
    JMX_MEM=$(echo "$JMX_STATS" | cut -d'/' -f1 | tr -d ' ')
    JAVA_MEM=$(echo "$JAVA_STATS" | cut -d'/' -f1 | tr -d ' ')
fi

echo "  rJMX-Exporter (Rust): $RJMX_MEM"
echo "  jmx_exporter (Java):  $JMX_MEM"
echo "  Java Test App:        $JAVA_MEM"
echo ""

echo "=============================================="
echo "   Response Time Comparison"
echo "=============================================="
echo ""

# Test both endpoints
RJMX_LATENCY='{"error": "not tested"}'
JMX_LATENCY='{"error": "not tested"}'

# Test rJMX-Exporter
if curl -s "$RJMX_URL" > /dev/null 2>&1; then
    echo ""
    echo "  rJMX-Exporter:"
    RJMX_LATENCY=$(measure_latency "$RJMX_URL" "rJMX-Exporter" $TEST_REQUESTS | tail -1)
else
    log_warn "rJMX-Exporter not available at $RJMX_URL"
    RJMX_LATENCY='{"error": "not available"}'
fi

# Test jmx_exporter
if curl -s "$JMX_URL" > /dev/null 2>&1; then
    echo ""
    echo "  jmx_exporter:"
    JMX_LATENCY=$(measure_latency "$JMX_URL" "jmx_exporter" $TEST_REQUESTS | tail -1)
else
    log_warn "jmx_exporter not available at $JMX_URL"
    JMX_LATENCY='{"error": "not available"}'
fi
echo ""

# Load test with wrk (if available and not quick mode)
RJMX_WRK=""
JMX_WRK=""

if [[ "$HAS_WRK" == "true" && "$QUICK_MODE" == "false" ]]; then
    echo "=============================================="
    echo "   Load Test (wrk - ${TEST_DURATION}s)"
    echo "=============================================="
    echo ""

    log_info "Running load test on rJMX-Exporter..."
    RJMX_WRK=$(wrk -t$THREADS -c$CONCURRENT_CONNECTIONS -d${TEST_DURATION}s --latency "$RJMX_URL" 2>&1)
    echo "$RJMX_WRK" | tail -15
    echo ""

    log_info "Running load test on jmx_exporter..."
    JMX_WRK=$(wrk -t$THREADS -c$CONCURRENT_CONNECTIONS -d${TEST_DURATION}s --latency "$JMX_URL" 2>&1)
    echo "$JMX_WRK" | tail -15
    echo ""
fi

# Metrics output comparison
echo "=============================================="
echo "   Metrics Output Comparison"
echo "=============================================="
echo ""

RJMX_LINES=0
JMX_LINES=0
RJMX_SIZE=0
JMX_SIZE=0

if curl -s "$RJMX_URL" > /dev/null 2>&1; then
    RJMX_METRICS=$(curl -sf "$RJMX_URL")
    RJMX_LINES=$(echo "$RJMX_METRICS" | wc -l | tr -d ' ')
    RJMX_SIZE=$(echo "$RJMX_METRICS" | wc -c | tr -d ' ')
fi

if curl -s "$JMX_URL" > /dev/null 2>&1; then
    JMX_METRICS=$(curl -sf "$JMX_URL")
    JMX_LINES=$(echo "$JMX_METRICS" | wc -l | tr -d ' ')
    JMX_SIZE=$(echo "$JMX_METRICS" | wc -c | tr -d ' ')
fi

echo "  rJMX-Exporter: $RJMX_LINES lines, $RJMX_SIZE bytes"
echo "  jmx_exporter:  $JMX_LINES lines, $JMX_SIZE bytes"
echo ""

# Sample output
if [[ -n "$RJMX_METRICS" ]]; then
    log_info "Sample metrics from rJMX-Exporter:"
    echo "$RJMX_METRICS" | grep -E "^jvm_" | head -5 || echo "  (no jvm_ metrics found)"
    echo ""
fi

if [[ -n "$JMX_METRICS" ]]; then
    log_info "Sample metrics from jmx_exporter:"
    echo "$JMX_METRICS" | grep -E "^jvm_" | head -5 || echo "  (no jvm_ metrics found)"
    echo ""
fi

# Save results to JSON
log_info "Saving results to $RESULTS_FILE..."

cat > "$RESULTS_FILE" << EOF
{
  "timestamp": "$(date -Iseconds)",
  "environment": {
    "cpu_limit": "1 core",
    "memory_limit": "512MB",
    "warmup_requests": $WARMUP_REQUESTS,
    "test_requests": $TEST_REQUESTS,
    "test_duration_seconds": $TEST_DURATION,
    "concurrent_connections": $CONCURRENT_CONNECTIONS
  },
  "results": {
    "memory": {
      "rjmx_exporter": "$RJMX_MEM",
      "jmx_exporter": "$JMX_MEM",
      "java_app": "$JAVA_MEM"
    },
    "latency": {
      "rjmx_exporter": $RJMX_LATENCY,
      "jmx_exporter": $JMX_LATENCY
    },
    "metrics_output": {
      "rjmx_exporter": {
        "lines": $RJMX_LINES,
        "bytes": $RJMX_SIZE
      },
      "jmx_exporter": {
        "lines": $JMX_LINES,
        "bytes": $JMX_SIZE
      }
    }
  }
}
EOF

log_success "Results saved to $RESULTS_FILE"

echo ""
echo "=============================================="
echo "   Summary"
echo "=============================================="
echo ""
echo "  Memory Usage:"
echo "    rJMX-Exporter (Rust): $RJMX_MEM"
echo "    jmx_exporter (Java):  $JMX_MEM"
echo ""

# Performance comparison
if [[ "$RJMX_LATENCY" != *"error"* ]] && [[ "$JMX_LATENCY" != *"error"* ]]; then
    if [[ "$HAS_JQ" == "true" ]]; then
        RJMX_P50=$(echo "$RJMX_LATENCY" | jq -r '.p50 // empty')
        JMX_P50=$(echo "$JMX_LATENCY" | jq -r '.p50 // empty')
        RJMX_AVG=$(echo "$RJMX_LATENCY" | jq -r '.avg // empty')
        JMX_AVG=$(echo "$JMX_LATENCY" | jq -r '.avg // empty')

        echo "  Latency (P50):"
        echo "    rJMX-Exporter: ${RJMX_P50}ms"
        echo "    jmx_exporter:  ${JMX_P50}ms"

        if [ -n "$RJMX_P50" ] && [ -n "$JMX_P50" ]; then
            SPEEDUP=$(echo "scale=2; $JMX_P50 / $RJMX_P50" | bc 2>/dev/null || echo "N/A")
            echo "    Speedup: ${SPEEDUP}x"
        fi
    fi
fi
echo ""
echo "=============================================="

# Cleanup option
if [[ "$USE_DOCKER" == "true" && "$HAS_DOCKER" == "true" ]]; then
    echo ""
    read -p "Stop benchmark containers? [y/N] " -n 1 -r
    echo ""
    if [[ $REPLY =~ ^[Yy]$ ]]; then
        log_info "Stopping containers..."
        cd "$PROJECT_DIR"
        docker-compose -f docker-compose.benchmark.yaml down -v
        log_success "Containers stopped"
    else
        log_info "Containers are still running."
        echo "  To stop: docker-compose -f docker-compose.benchmark.yaml down -v"
    fi
fi

echo ""
echo "Benchmark complete!"
