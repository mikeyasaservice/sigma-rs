#!/bin/bash
# Health check script for Kafka/Redpanda
# Verifies that Kafka is running and accessible

set -e

# Colors for output
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

# Configuration
REDPANDA_CONTAINER="redpanda-0"
KAFKA_INTERNAL_PORT="9092"
KAFKA_EXTERNAL_PORT="19092"
REQUIRED_TOPICS=("security-events" "sigma-matches" "dlq-events")
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

# Exit codes
EXIT_SUCCESS=0
EXIT_CONTAINER_NOT_RUNNING=1
EXIT_KAFKA_NOT_RESPONSIVE=2
EXIT_TOPICS_MISSING=3
EXIT_CANNOT_PRODUCE=4

# Health check results
CHECKS_PASSED=0
CHECKS_FAILED=0

# Function to perform a health check
perform_check() {
    local check_name="$1"
    local check_command="$2"
    local failure_exit_code="${3:-$EXIT_SUCCESS}"
    
    echo -ne "${YELLOW}Checking: ${check_name}...${NC} "
    
    if eval "$check_command" >/dev/null 2>&1; then
        echo -e "${GREEN}✓${NC}"
        CHECKS_PASSED=$((CHECKS_PASSED + 1))
        return 0
    else
        echo -e "${RED}✗${NC}"
        CHECKS_FAILED=$((CHECKS_FAILED + 1))
        
        if [ "$failure_exit_code" -ne 0 ] && [ "${FAIL_FAST:-false}" = "true" ]; then
            exit $failure_exit_code
        fi
        return 1
    fi
}

# Check 1: Container is running
check_container() {
    docker ps --format "{{.Names}}" | grep -q "^${REDPANDA_CONTAINER}$"
}

# Check 2: Kafka cluster is healthy
check_cluster_health() {
    docker exec "$REDPANDA_CONTAINER" rpk cluster health --exit-when-healthy
}

# Check 3: Kafka broker is responsive
check_broker_responsive() {
    docker exec "$REDPANDA_CONTAINER" rpk cluster info --brokers "localhost:${KAFKA_INTERNAL_PORT}"
}

# Check 4: External port is accessible
check_external_port() {
    nc -zv localhost "$KAFKA_EXTERNAL_PORT" 2>&1 | grep -q "succeeded"
}

# Check 5: Required topics exist
check_topics_exist() {
    local topics_list=$(docker exec "$REDPANDA_CONTAINER" rpk topic list 2>/dev/null)
    
    for topic in "${REQUIRED_TOPICS[@]}"; do
        if ! echo "$topics_list" | grep -q "${topic}"; then
            return 1
        fi
    done
    return 0
}

# Check 6: Can produce messages
check_can_produce() {
    echo "health-check-$(date +%s)" | docker exec -i "$REDPANDA_CONTAINER" \
        rpk topic produce "security-events" --brokers "localhost:${KAFKA_INTERNAL_PORT}"
}

# Check 7: Can list consumer groups
check_consumer_groups() {
    docker exec "$REDPANDA_CONTAINER" rpk group list --brokers "localhost:${KAFKA_INTERNAL_PORT}"
}

# Function to display detailed status
display_detailed_status() {
    echo ""
    echo -e "${BLUE}Detailed Status Information:${NC}"
    echo "========================================="
    
    # Show container status
    echo -e "${BLUE}Container Status:${NC}"
    docker ps --filter "name=${REDPANDA_CONTAINER}" --format "table {{.Names}}\t{{.Status}}\t{{.Ports}}" 2>/dev/null || echo "Container not found"
    echo ""
    
    # Show cluster information
    if docker exec "$REDPANDA_CONTAINER" rpk cluster info --brokers "localhost:${KAFKA_INTERNAL_PORT}" >/dev/null 2>&1; then
        echo -e "${BLUE}Cluster Information:${NC}"
        docker exec "$REDPANDA_CONTAINER" rpk cluster info --brokers "localhost:${KAFKA_INTERNAL_PORT}" 2>/dev/null
        echo ""
    fi
    
    # Show topics
    if docker exec "$REDPANDA_CONTAINER" rpk topic list >/dev/null 2>&1; then
        echo -e "${BLUE}Topics:${NC}"
        docker exec "$REDPANDA_CONTAINER" rpk topic list 2>/dev/null
        echo ""
    fi
    
    # Show consumer groups
    if docker exec "$REDPANDA_CONTAINER" rpk group list --brokers "localhost:${KAFKA_INTERNAL_PORT}" >/dev/null 2>&1; then
        echo -e "${BLUE}Consumer Groups:${NC}"
        docker exec "$REDPANDA_CONTAINER" rpk group list --brokers "localhost:${KAFKA_INTERNAL_PORT}" 2>/dev/null
        echo ""
    fi
}

# Function to suggest fixes
suggest_fixes() {
    if [ $CHECKS_FAILED -gt 0 ]; then
        echo ""
        echo -e "${YELLOW}Suggested Fixes:${NC}"
        echo "----------------------------------------"
        
        if ! check_container >/dev/null 2>&1; then
            echo "• Container not running:"
            echo "  Run: docker-compose up -d"
            echo ""
        fi
        
        if ! check_topics_exist >/dev/null 2>&1; then
            echo "• Required topics missing:"
            echo "  Run: ./scripts/setup-kafka.sh"
            echo ""
        fi
        
        if ! check_external_port >/dev/null 2>&1; then
            echo "• External port not accessible:"
            echo "  Check if port 19092 is available"
            echo "  Check firewall settings"
            echo ""
        fi
    fi
}

# Main health check function
main() {
    echo "========================================="
    echo -e "${BLUE}Kafka/Redpanda Health Check${NC}"
    echo "========================================="
    echo ""
    
    # Change to project root
    cd "$PROJECT_ROOT"
    
    # Perform health checks
    perform_check "Container running" check_container $EXIT_CONTAINER_NOT_RUNNING
    perform_check "Cluster healthy" check_cluster_health $EXIT_KAFKA_NOT_RESPONSIVE
    perform_check "Broker responsive" check_broker_responsive $EXIT_KAFKA_NOT_RESPONSIVE
    perform_check "External port accessible" check_external_port
    perform_check "Required topics exist" check_topics_exist $EXIT_TOPICS_MISSING
    perform_check "Can produce messages" check_can_produce $EXIT_CANNOT_PRODUCE
    perform_check "Consumer groups accessible" check_consumer_groups
    
    echo ""
    echo "========================================="
    echo -e "${BLUE}Health Check Summary${NC}"
    echo "========================================="
    echo -e "Checks Passed: ${GREEN}${CHECKS_PASSED}${NC}"
    echo -e "Checks Failed: ${RED}${CHECKS_FAILED}${NC}"
    echo ""
    
    # Display detailed status if verbose mode
    if [ "${VERBOSE:-false}" = "true" ]; then
        display_detailed_status
    fi
    
    # Suggest fixes if there were failures
    if [ $CHECKS_FAILED -gt 0 ]; then
        suggest_fixes
        echo -e "${RED}✗ Health check failed${NC}"
        
        # Exit with appropriate code
        if [ "${FAIL_FAST:-false}" = "true" ]; then
            exit $EXIT_KAFKA_NOT_RESPONSIVE
        else
            exit 1
        fi
    else
        echo -e "${GREEN}✓ All health checks passed!${NC}"
        echo ""
        echo "Kafka is ready for use:"
        echo "  - Internal: localhost:9092 (Docker)"
        echo "  - External: localhost:19092 (Host)"
        echo ""
        exit $EXIT_SUCCESS
    fi
}

# Handle script arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        --verbose|-v)
            export VERBOSE=true
            shift
            ;;
        --fail-fast)
            export FAIL_FAST=true
            shift
            ;;
        --help)
            echo "Usage: $0 [OPTIONS]"
            echo ""
            echo "Options:"
            echo "  -v, --verbose    Show detailed status information"
            echo "  --fail-fast      Exit immediately on first failure"
            echo "  --help           Show this help message"
            echo ""
            echo "Exit codes:"
            echo "  0 - All checks passed"
            echo "  1 - Container not running"
            echo "  2 - Kafka not responsive"
            echo "  3 - Required topics missing"
            echo "  4 - Cannot produce messages"
            exit 0
            ;;
        *)
            echo "Unknown option: $1"
            echo "Use --help for usage information"
            exit 1
            ;;
    esac
done

# Run main function
main