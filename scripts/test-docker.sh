#!/bin/bash
#
# Docker Integration Test Script for rJMX-Exporter
#
# This script runs the complete Docker-based integration test suite:
# 1. Builds the test Java application with Jolokia
# 2. Builds the rJMX-Exporter container
# 3. Runs integration tests
# 4. Reports results and cleans up
#
# Usage:
#   ./scripts/test-docker.sh [--keep]
#
# Options:
#   --keep    Keep containers running after tests (for debugging)
#
# Requirements:
#   - Docker
#   - Docker Compose

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Script directory
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"
COMPOSE_FILE="$PROJECT_DIR/tests/docker-compose.test.yaml"

# Parse arguments
KEEP_CONTAINERS=false
while [[ $# -gt 0 ]]; do
    case $1 in
        --keep)
            KEEP_CONTAINERS=true
            shift
            ;;
        *)
            echo "Unknown option: $1"
            echo "Usage: $0 [--keep]"
            exit 1
            ;;
    esac
done

echo -e "${YELLOW}=== rJMX-Exporter Docker Integration Tests ===${NC}"
echo ""

# Check prerequisites
if ! command -v docker &> /dev/null; then
    echo -e "${RED}Error: Docker is not installed${NC}"
    exit 1
fi

if ! command -v docker-compose &> /dev/null && ! docker compose version &> /dev/null; then
    echo -e "${RED}Error: Docker Compose is not installed${NC}"
    exit 1
fi

# Determine docker compose command
if docker compose version &> /dev/null; then
    COMPOSE_CMD="docker compose"
else
    COMPOSE_CMD="docker-compose"
fi

# Function to cleanup
cleanup() {
    if [ "$KEEP_CONTAINERS" = false ]; then
        echo ""
        echo -e "${YELLOW}Cleaning up containers...${NC}"
        $COMPOSE_CMD -f "$COMPOSE_FILE" down -v --remove-orphans 2>/dev/null || true
    else
        echo ""
        echo -e "${YELLOW}Keeping containers running (--keep flag)${NC}"
        echo "To clean up manually, run:"
        echo "  $COMPOSE_CMD -f $COMPOSE_FILE down -v"
    fi
}

# Set trap to cleanup on exit
trap cleanup EXIT

# Clean up any previous test containers
echo "Cleaning up previous test containers..."
$COMPOSE_CMD -f "$COMPOSE_FILE" down -v --remove-orphans 2>/dev/null || true

# Build and run tests
echo ""
echo -e "${YELLOW}Building and starting test environment...${NC}"
echo ""

# Run the integration tests
# --build: Rebuild images
# --abort-on-container-exit: Stop all containers when test-runner exits
# --exit-code-from test-runner: Use test-runner's exit code as our exit code
if $COMPOSE_CMD -f "$COMPOSE_FILE" up \
    --build \
    --abort-on-container-exit \
    --exit-code-from test-runner; then

    echo ""
    echo -e "${GREEN}=== All Docker Integration Tests Passed ===${NC}"
    EXIT_CODE=0
else
    echo ""
    echo -e "${RED}=== Docker Integration Tests Failed ===${NC}"
    EXIT_CODE=1

    # Show logs on failure
    echo ""
    echo -e "${YELLOW}Container logs:${NC}"
    $COMPOSE_CMD -f "$COMPOSE_FILE" logs --tail=50
fi

exit $EXIT_CODE
