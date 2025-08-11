#!/bin/bash
# Test suite for sigma-rs service mode
# TDD approach - write tests before implementation

set -e

# Colors for output
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
NC='\033[0m'

# Test configuration
SERVICE_BINARY="./target/release/sigma-rs-service"
SERVICE_PORT=8080
GRPC_PORT=9090
API_KEY="test-api-key"

# Test counters
TESTS_RUN=0
TESTS_PASSED=0
TESTS_FAILED=0

# Helper function to run tests
run_test() {
    local test_name="$1"
    local test_command="$2"
    
    TESTS_RUN=$((TESTS_RUN + 1))
    echo -e "${YELLOW}Running: $test_name${NC}"
    
    if eval "$test_command"; then
        echo -e "${GREEN}✓ PASSED: $test_name${NC}"
        TESTS_PASSED=$((TESTS_PASSED + 1))
        return 0
    else
        echo -e "${RED}✗ FAILED: $test_name${NC}"
        TESTS_FAILED=$((TESTS_FAILED + 1))
        return 1
    fi
}

# Test 1: Service binary exists
test_service_binary_exists() {
    [ -f "$SERVICE_BINARY" ] && [ -x "$SERVICE_BINARY" ]
}

# Test 2: Service binary shows help
test_service_help() {
    $SERVICE_BINARY --help 2>&1 | grep -q "sigma-rs-service"
}

# Test 3: Service can be started
test_service_starts() {
    # Start service in background
    export SIGMA_API_KEY="$API_KEY"
    $SERVICE_BINARY --rules ./rules --http-port $SERVICE_PORT &
    local service_pid=$!
    
    # Wait for service to start
    sleep 3
    
    # Check if process is running
    if kill -0 $service_pid 2>/dev/null; then
        # Clean up
        kill $service_pid 2>/dev/null || true
        wait $service_pid 2>/dev/null || true
        return 0
    else
        return 1
    fi
}

# Test 4: Health endpoint responds
test_health_endpoint() {
    # Start service
    export SIGMA_API_KEY="$API_KEY"
    $SERVICE_BINARY --rules ./rules --http-port $SERVICE_PORT &
    local service_pid=$!
    
    # Wait for service to start
    sleep 3
    
    # Test health endpoint
    local response=$(curl -s -o /dev/null -w "%{http_code}" http://localhost:$SERVICE_PORT/health)
    
    # Clean up
    kill $service_pid 2>/dev/null || true
    wait $service_pid 2>/dev/null || true
    
    [ "$response" = "200" ]
}

# Test 5: Metrics endpoint responds
test_metrics_endpoint() {
    # Start service
    export SIGMA_API_KEY="$API_KEY"
    $SERVICE_BINARY --rules ./rules --http-port $SERVICE_PORT &
    local service_pid=$!
    
    # Wait for service to start
    sleep 3
    
    # Test metrics endpoint with API key
    local response=$(curl -s -o /dev/null -w "%{http_code}" \
        -H "x-api-key: $API_KEY" \
        http://localhost:$SERVICE_PORT/metrics)
    
    # Clean up
    kill $service_pid 2>/dev/null || true
    wait $service_pid 2>/dev/null || true
    
    [ "$response" = "200" ]
}

# Test 6: Evaluate endpoint processes events
test_evaluate_endpoint() {
    # Start service
    export SIGMA_API_KEY="$API_KEY"
    $SERVICE_BINARY --rules ./rules --http-port $SERVICE_PORT &
    local service_pid=$!
    
    # Wait for service to start
    sleep 3
    
    # Test evaluate endpoint
    local response=$(curl -s -o /dev/null -w "%{http_code}" \
        -X POST \
        -H "x-api-key: $API_KEY" \
        -H "Content-Type: application/json" \
        -d '{"EventID": 1, "Image": "C:\\Windows\\System32\\cmd.exe", "CommandLine": "cmd.exe /c whoami"}' \
        http://localhost:$SERVICE_PORT/evaluate)
    
    # Clean up
    kill $service_pid 2>/dev/null || true
    wait $service_pid 2>/dev/null || true
    
    [ "$response" = "200" ]
}

# Test 7: API key authentication works
test_api_key_auth() {
    # Start service with API key requirement
    export SIGMA_API_KEY="$API_KEY"
    $SERVICE_BINARY --rules ./rules --http-port $SERVICE_PORT &
    local service_pid=$!
    
    # Wait for service to start
    sleep 3
    
    # Test without API key (should fail)
    local no_key_response=$(curl -s -o /dev/null -w "%{http_code}" \
        http://localhost:$SERVICE_PORT/metrics)
    
    # Test with wrong API key (should fail)
    local wrong_key_response=$(curl -s -o /dev/null -w "%{http_code}" \
        -H "x-api-key: wrong-key" \
        http://localhost:$SERVICE_PORT/metrics)
    
    # Test with correct API key (should succeed)
    local correct_key_response=$(curl -s -o /dev/null -w "%{http_code}" \
        -H "x-api-key: $API_KEY" \
        http://localhost:$SERVICE_PORT/metrics)
    
    # Clean up
    kill $service_pid 2>/dev/null || true
    wait $service_pid 2>/dev/null || true
    
    # Verify auth behavior
    [ "$no_key_response" = "401" ] && \
    [ "$wrong_key_response" = "401" ] && \
    [ "$correct_key_response" = "200" ]
}

# Test 8: Service handles graceful shutdown
test_graceful_shutdown() {
    # Start service
    export SIGMA_API_KEY="$API_KEY"
    $SERVICE_BINARY --rules ./rules --http-port $SERVICE_PORT &
    local service_pid=$!
    
    # Wait for service to start
    sleep 3
    
    # Send SIGTERM for graceful shutdown
    kill -TERM $service_pid 2>/dev/null
    
    # Wait for process to exit (with timeout)
    local count=0
    while kill -0 $service_pid 2>/dev/null && [ $count -lt 10 ]; do
        sleep 1
        count=$((count + 1))
    done
    
    # Check if process exited cleanly
    if ! kill -0 $service_pid 2>/dev/null; then
        return 0
    else
        # Force kill if still running
        kill -9 $service_pid 2>/dev/null || true
        return 1
    fi
}

# Test 9: Service loads rules correctly
test_rule_loading() {
    # Ensure we have test rules
    if [ ! -f "./rules/test-process.yml" ]; then
        echo "Test rules not found"
        return 1
    fi
    
    # Start service
    export SIGMA_API_KEY="$API_KEY"
    $SERVICE_BINARY --rules ./rules --http-port $SERVICE_PORT &
    local service_pid=$!
    
    # Wait for service to start
    sleep 3
    
    # Check rules endpoint
    local response=$(curl -s -H "x-api-key: $API_KEY" http://localhost:$SERVICE_PORT/rules)
    
    # Clean up
    kill $service_pid 2>/dev/null || true
    wait $service_pid 2>/dev/null || true
    
    # Check if response contains rule information
    echo "$response" | grep -q "rules"
}

# Test 10: Service can run with config file
test_config_file() {
    # Create test config
    cat > test-service.toml <<EOF
[service]
http_port = 8081
grpc_port = 9091

[engine]
rules_dir = "./rules"
EOF
    
    # Start service with config
    export SIGMA_API_KEY="$API_KEY"
    $SERVICE_BINARY --config test-service.toml &
    local service_pid=$!
    
    # Wait for service to start
    sleep 3
    
    # Test on configured port
    local response=$(curl -s -o /dev/null -w "%{http_code}" http://localhost:8081/health)
    
    # Clean up
    kill $service_pid 2>/dev/null || true
    wait $service_pid 2>/dev/null || true
    rm -f test-service.toml
    
    [ "$response" = "200" ]
}

# Main test execution
main() {
    echo "========================================="
    echo "Sigma-rs Service Mode Test Suite"
    echo "========================================="
    echo ""
    
    # Check if we're in the right directory
    if [ ! -f "Cargo.toml" ]; then
        echo -e "${RED}Error: Must run from sigma-rs project root${NC}"
        exit 1
    fi
    
    # Build the service binary first
    echo -e "${YELLOW}Building service binary...${NC}"
    if cargo build --release --features service --bin sigma-rs-service 2>/dev/null; then
        echo -e "${GREEN}✓ Build successful${NC}"
    else
        echo -e "${RED}✗ Build failed (expected for TDD)${NC}"
    fi
    echo ""
    
    # Run all tests
    run_test "Service binary exists" test_service_binary_exists || true
    run_test "Service shows help" test_service_help || true
    run_test "Service starts successfully" test_service_starts || true
    run_test "Health endpoint responds" test_health_endpoint || true
    run_test "Metrics endpoint responds" test_metrics_endpoint || true
    run_test "Evaluate endpoint works" test_evaluate_endpoint || true
    run_test "API key authentication" test_api_key_auth || true
    run_test "Graceful shutdown" test_graceful_shutdown || true
    run_test "Rules load correctly" test_rule_loading || true
    run_test "Config file support" test_config_file || true
    
    echo ""
    echo "========================================="
    echo "Test Results"
    echo "========================================="
    echo -e "Tests Run: $TESTS_RUN"
    echo -e "${GREEN}Tests Passed: $TESTS_PASSED${NC}"
    echo -e "${RED}Tests Failed: $TESTS_FAILED${NC}"
    echo ""
    
    if [ $TESTS_FAILED -eq 0 ]; then
        echo -e "${GREEN}✓ All tests passed!${NC}"
        exit 0
    else
        echo -e "${RED}✗ Some tests failed${NC}"
        echo ""
        echo "Implementation needed for:"
        echo "  - Create src/bin/sigma-rs-service.rs"
        echo "  - Import service components from library"
        echo "  - Parse CLI arguments"
        echo "  - Start HTTP/gRPC servers"
        exit 1
    fi
}

# Run tests if script is executed directly
if [ "${BASH_SOURCE[0]}" = "${0}" ]; then
    main "$@"
fi