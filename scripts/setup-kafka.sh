#!/bin/bash
# Setup script for Kafka/Redpanda topics and configurations
# Creates required topics for sigma-rs with appropriate settings

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
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

# Topic configurations (using simple arrays for compatibility)
TOPIC_NAMES=("security-events" "sigma-matches" "dlq-events")
TOPIC_CONFIGS=(
    "--partitions 3 --replicas 1 --config retention.ms=86400000"   # security-events: 1 day
    "--partitions 3 --replicas 1 --config retention.ms=604800000"  # sigma-matches: 7 days  
    "--partitions 1 --replicas 1 --config retention.ms=2592000000" # dlq-events: 30 days
)

# Optional topics
OPTIONAL_TOPIC_NAMES=("events" "sigma-events")
OPTIONAL_TOPIC_CONFIGS=(
    "--partitions 3 --replicas 1 --config retention.ms=86400000"
    "--partitions 3 --replicas 1 --config retention.ms=86400000"
)

# Function to check if Redpanda is running
check_redpanda_running() {
    if ! docker ps --format "{{.Names}}" | grep -q "^${REDPANDA_CONTAINER}$"; then
        echo -e "${YELLOW}Redpanda not running. Starting it now...${NC}"
        
        # Start Redpanda using docker-compose
        if [ -f "$PROJECT_ROOT/docker-compose.yml" ]; then
            cd "$PROJECT_ROOT"
            docker-compose up -d
            echo -e "${GREEN}✓ Started Redpanda${NC}"
            
            # Wait for Redpanda to be ready
            echo -e "${YELLOW}Waiting for Redpanda to be ready...${NC}"
            sleep 10  # Initial wait
            
            local retries=30
            while [ $retries -gt 0 ]; do
                if docker exec "$REDPANDA_CONTAINER" rpk cluster health --exit-when-healthy >/dev/null 2>&1; then
                    echo -e "${GREEN}✓ Redpanda is healthy${NC}"
                    break
                fi
                echo -n "."
                sleep 2
                retries=$((retries - 1))
            done
            
            if [ $retries -eq 0 ]; then
                echo -e "${RED}✗ Redpanda failed to become healthy${NC}"
                exit 1
            fi
        else
            echo -e "${RED}✗ docker-compose.yml not found${NC}"
            exit 1
        fi
    else
        echo -e "${GREEN}✓ Redpanda is already running${NC}"
    fi
}

# Function to wait for Kafka to be ready
wait_for_kafka() {
    echo -e "${YELLOW}Checking Kafka connectivity...${NC}"
    local retries=10
    
    while [ $retries -gt 0 ]; do
        if docker exec "$REDPANDA_CONTAINER" rpk cluster info --brokers "localhost:${KAFKA_INTERNAL_PORT}" >/dev/null 2>&1; then
            echo -e "${GREEN}✓ Kafka is responsive${NC}"
            return 0
        fi
        echo -n "."
        sleep 2
        retries=$((retries - 1))
    done
    
    echo -e "${RED}✗ Kafka is not responding${NC}"
    return 1
}

# Function to create a topic
create_topic() {
    local topic_name="$1"
    local topic_config="$2"
    
    # Check if topic already exists
    if docker exec "$REDPANDA_CONTAINER" rpk topic list 2>/dev/null | grep -q "^${topic_name}$"; then
        echo -e "${BLUE}ℹ Topic '${topic_name}' already exists (idempotent)${NC}"
        
        # Update retention if needed (idempotent operation)
        if [[ "$topic_config" == *"retention.ms"* ]]; then
            local retention=$(echo "$topic_config" | grep -o 'retention.ms=[0-9]*' | cut -d= -f2)
            docker exec "$REDPANDA_CONTAINER" rpk topic alter-config "$topic_name" \
                --set retention.ms="$retention" >/dev/null 2>&1 || true
        fi
    else
        echo -e "${YELLOW}Creating topic: ${topic_name}${NC}"
        
        # Create the topic with configuration
        if docker exec "$REDPANDA_CONTAINER" rpk topic create "$topic_name" $topic_config >/dev/null 2>&1; then
            echo -e "${GREEN}✓ Created topic: ${topic_name}${NC}"
        else
            echo -e "${RED}✗ Failed to create topic: ${topic_name}${NC}"
            # Don't return error - might already exist
        fi
    fi
    
    # Verify topic was created
    if docker exec "$REDPANDA_CONTAINER" rpk topic describe "$topic_name" >/dev/null 2>&1; then
        return 0
    else
        echo -e "${RED}✗ Topic verification failed: ${topic_name}${NC}"
        return 1
    fi
}

# Function to display topic information
display_topic_info() {
    echo ""
    echo -e "${BLUE}Topic Configuration Summary:${NC}"
    echo "========================================="
    
    for topic in "${TOPIC_NAMES[@]}"; do
        if docker exec "$REDPANDA_CONTAINER" rpk topic list 2>/dev/null | grep -q "^${topic}$"; then
            echo -e "${GREEN}✓ ${topic}${NC}"
            docker exec "$REDPANDA_CONTAINER" rpk topic describe "$topic" 2>/dev/null | grep -E "PARTITIONS|REPLICAS" | head -1
            docker exec "$REDPANDA_CONTAINER" rpk topic describe "$topic" -c 2>/dev/null | grep "retention.ms" | head -1 || echo "  No retention config"
            echo ""
        fi
    done
}

# Function to test topic functionality
test_topic_functionality() {
    echo -e "${YELLOW}Testing topic functionality...${NC}"
    
    local test_topic="security-events"
    local test_message="test-$(date +%s)"
    
    # Test producing
    if echo "$test_message" | docker exec -i "$REDPANDA_CONTAINER" \
        rpk topic produce "$test_topic" --brokers "localhost:${KAFKA_INTERNAL_PORT}" >/dev/null 2>&1; then
        echo -e "${GREEN}✓ Can produce to ${test_topic}${NC}"
    else
        echo -e "${RED}✗ Cannot produce to ${test_topic}${NC}"
        return 1
    fi
    
    # Test consuming (with timeout)
    if timeout 5 docker exec "$REDPANDA_CONTAINER" \
        rpk topic consume "$test_topic" --brokers "localhost:${KAFKA_INTERNAL_PORT}" --num 1 >/dev/null 2>&1; then
        echo -e "${GREEN}✓ Can consume from ${test_topic}${NC}"
    else
        echo -e "${YELLOW}⚠ Consume test timed out (expected for empty topic)${NC}"
    fi
    
    return 0
}

# Main setup function
main() {
    echo "========================================="
    echo -e "${BLUE}Sigma-rs Kafka/Redpanda Setup${NC}"
    echo "========================================="
    echo ""
    
    # Change to project root
    cd "$PROJECT_ROOT"
    
    # Step 1: Ensure Redpanda is running
    check_redpanda_running
    
    # Step 2: Wait for Kafka to be ready
    wait_for_kafka
    
    # Step 3: Create required topics
    echo ""
    echo -e "${BLUE}Creating required topics...${NC}"
    echo "----------------------------------------"
    
    for i in "${!TOPIC_NAMES[@]}"; do
        create_topic "${TOPIC_NAMES[$i]}" "${TOPIC_CONFIGS[$i]}"
    done
    
    # Step 4: Optionally create additional topics
    if [ "${CREATE_OPTIONAL_TOPICS:-false}" = "true" ]; then
        echo ""
        echo -e "${BLUE}Creating optional topics...${NC}"
        echo "----------------------------------------"
        
        for i in "${!OPTIONAL_TOPIC_NAMES[@]}"; do
            create_topic "${OPTIONAL_TOPIC_NAMES[$i]}" "${OPTIONAL_TOPIC_CONFIGS[$i]}"
        done
    fi
    
    # Step 5: Display configuration summary
    display_topic_info
    
    # Step 6: Test functionality
    test_topic_functionality
    
    echo ""
    echo "========================================="
    echo -e "${GREEN}✓ Kafka setup complete!${NC}"
    echo "========================================="
    echo ""
    echo "Topics created:"
    for topic in "${TOPIC_NAMES[@]}"; do
        echo "  - $topic"
    done
    echo ""
    echo "Kafka brokers:"
    echo "  - Internal: localhost:9092 (within Docker)"
    echo "  - External: localhost:19092 (from host)"
    echo ""
    echo "Next steps:"
    echo "  1. Run: ./scripts/check-kafka.sh to verify health"
    echo "  2. Run: cargo run --features kafka to test consumer"
    echo ""
}

# Handle script arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        --optional)
            export CREATE_OPTIONAL_TOPICS=true
            shift
            ;;
        --help)
            echo "Usage: $0 [--optional]"
            echo ""
            echo "Options:"
            echo "  --optional    Also create optional development topics"
            echo "  --help        Show this help message"
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