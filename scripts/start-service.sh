#!/bin/bash
# Start script for sigma-rs service

set -e

# Colors for output
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

# Configuration
SERVICE_BINARY="./target/release/sigma-rs-service"
DEFAULT_PORT=8088
DEFAULT_GRPC_PORT=9090
DEFAULT_RULES="./rules"

# Parse arguments
HTTP_PORT=${HTTP_PORT:-$DEFAULT_PORT}
GRPC_PORT=${GRPC_PORT:-$DEFAULT_GRPC_PORT}
RULES_DIR=${RULES_DIR:-$DEFAULT_RULES}
DEBUG=${DEBUG:-false}

# Check if service binary exists
if [ ! -f "$SERVICE_BINARY" ]; then
    echo -e "${YELLOW}Service binary not found. Building...${NC}"
    cargo build --release --features service --bin sigma-rs-service
fi

# Check if rules directory exists
if [ ! -d "$RULES_DIR" ]; then
    echo -e "${RED}Error: Rules directory not found: $RULES_DIR${NC}"
    exit 1
fi

# Set API key if not already set
if [ -z "$SIGMA_API_KEY" ]; then
    export SIGMA_API_KEY="change-me-in-production"
    echo -e "${YELLOW}Warning: Using default API key. Set SIGMA_API_KEY for production.${NC}"
fi

echo "========================================="
echo -e "${BLUE}Starting Sigma-rs Service${NC}"
echo "========================================="
echo ""
echo "Configuration:"
echo "  Rules: $RULES_DIR"
echo "  HTTP Port: $HTTP_PORT"
echo "  gRPC Port: $GRPC_PORT"
echo "  API Key: ${SIGMA_API_KEY:0:4}****"
echo ""
echo "Endpoints:"
echo "  Health: http://localhost:$HTTP_PORT/health"
echo "  Metrics: http://localhost:$HTTP_PORT/metrics"
echo "  Rules: http://localhost:$HTTP_PORT/rules"
echo "  Evaluate: http://localhost:$HTTP_PORT/evaluate"
echo ""
echo -e "${GREEN}Starting service...${NC}"
echo "Press Ctrl+C to stop"
echo ""

# Start the service
if [ "$DEBUG" = "true" ]; then
    exec $SERVICE_BINARY \
        --rules "$RULES_DIR" \
        --http-port "$HTTP_PORT" \
        --grpc-port "$GRPC_PORT" \
        --debug
else
    exec $SERVICE_BINARY \
        --rules "$RULES_DIR" \
        --http-port "$HTTP_PORT" \
        --grpc-port "$GRPC_PORT"
fi