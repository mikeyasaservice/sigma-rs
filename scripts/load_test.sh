#!/bin/bash
# Load testing script for sigma-rs

# Colors for output
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Default values
NUM_EVENTS=10000
BATCH_SIZE=1000
RULES_DIR="./rules"
OUTPUT_DIR="./load_test_results"
TIMESTAMP=$(date +%Y%m%d_%H%M%S)

# Parse arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        --events)
            NUM_EVENTS="$2"
            shift 2
            ;;
        --batch)
            BATCH_SIZE="$2"
            shift 2
            ;;
        --rules)
            RULES_DIR="$2"
            shift 2
            ;;
        --help|-h)
            echo "Usage: ./scripts/load_test.sh [OPTIONS]"
            echo ""
            echo "Options:"
            echo "  --events <num>    Number of events to generate (default: 10000)"
            echo "  --batch <size>    Batch size for event generation (default: 1000)"
            echo "  --rules <dir>     Rules directory (default: ./rules)"
            echo "  --help            Show this help message"
            echo ""
            echo "Example:"
            echo "  ./scripts/load_test.sh --events 100000 --batch 5000"
            exit 0
            ;;
        *)
            echo "Unknown option: $1"
            exit 1
            ;;
    esac
done

# Create output directory
mkdir -p "$OUTPUT_DIR"

echo -e "${BLUE}=== Sigma-rs Load Test ===${NC}"
echo -e "Events: ${GREEN}$NUM_EVENTS${NC}"
echo -e "Batch Size: ${GREEN}$BATCH_SIZE${NC}"
echo -e "Rules Directory: ${GREEN}$RULES_DIR${NC}"
echo ""

# Check if rules directory exists
if [ ! -d "$RULES_DIR" ]; then
    echo -e "${RED}Error: Rules directory not found: $RULES_DIR${NC}"
    exit 1
fi

# Count rules
RULE_COUNT=$(find "$RULES_DIR" -name "*.yml" -o -name "*.yaml" | wc -l | tr -d ' ')
if [ "$RULE_COUNT" -eq 0 ]; then
    echo -e "${YELLOW}Warning: No rules found in $RULES_DIR${NC}"
    echo -e "${YELLOW}Creating sample rules for testing...${NC}"
    
    # Create sample rules for testing
    cat > "$RULES_DIR/test_rule_1.yml" << 'EOF'
title: High CPU Process
id: 12345678-1234-1234-1234-123456789001
detection:
  selection:
    EventID: 1
    CPU|gt: 80
  condition: selection
EOF

    cat > "$RULES_DIR/test_rule_2.yml" << 'EOF'
title: Network Connection to External IP
id: 12345678-1234-1234-1234-123456789002
detection:
  selection:
    EventID: 3
    DestinationIp|startswith: 
      - '10.'
      - '192.168.'
      - '172.'
  condition: not selection
EOF

    cat > "$RULES_DIR/test_rule_3.yml" << 'EOF'
title: Suspicious PowerShell Execution
id: 12345678-1234-1234-1234-123456789003
detection:
  selection:
    EventID: 1
    CommandLine|contains:
      - 'powershell'
      - 'pwsh'
      - 'PowerShell'
  suspicious:
    CommandLine|contains:
      - '-enc'
      - '-encoded'
      - 'bypass'
      - 'hidden'
  condition: selection and suspicious
EOF
    
    RULE_COUNT=3
fi

echo -e "Rules loaded: ${GREEN}$RULE_COUNT${NC}"
echo ""

# Generate test events
EVENT_FILE="$OUTPUT_DIR/events_${TIMESTAMP}.json"
echo -e "${BLUE}Generating $NUM_EVENTS test events...${NC}"

python3 - << EOF > "$EVENT_FILE"
import json
import random
import uuid
from datetime import datetime, timedelta

# Event templates
event_templates = [
    # Process creation events
    lambda i: {
        "EventID": 1,
        "ProcessId": random.randint(1000, 10000),
        "CommandLine": random.choice([
            "C:\\\\Windows\\\\System32\\\\cmd.exe /c dir",
            "powershell.exe -encoded SGVsbG8gV29ybGQ=",
            "python.exe script.py",
            "notepad.exe document.txt",
            "chrome.exe https://example.com"
        ]),
        "CPU": random.randint(0, 100),
        "Memory": random.randint(100000, 1000000000),
        "User": random.choice(["admin", "user1", "system", "guest"]),
        "Timestamp": (datetime.now() - timedelta(seconds=i)).isoformat()
    },
    # Network connection events
    lambda i: {
        "EventID": 3,
        "ProcessId": random.randint(1000, 10000),
        "SourceIp": f"192.168.1.{random.randint(1, 254)}",
        "DestinationIp": random.choice([
            f"10.0.0.{random.randint(1, 254)}",
            f"192.168.1.{random.randint(1, 254)}",
            f"172.16.0.{random.randint(1, 254)}",
            f"{random.randint(1, 223)}.{random.randint(0, 255)}.{random.randint(0, 255)}.{random.randint(1, 254)}"
        ]),
        "DestinationPort": random.choice([80, 443, 22, 3389, 8080]),
        "Protocol": random.choice(["TCP", "UDP"]),
        "Timestamp": (datetime.now() - timedelta(seconds=i)).isoformat()
    },
    # File creation events
    lambda i: {
        "EventID": 11,
        "ProcessId": random.randint(1000, 10000),
        "FileName": random.choice([
            "C:\\\\Users\\\\user\\\\Downloads\\\\file.exe",
            "C:\\\\Windows\\\\Temp\\\\tmp" + str(uuid.uuid4())[:8] + ".dat",
            "C:\\\\Program Files\\\\app\\\\config.ini",
            "C:\\\\Users\\\\user\\\\Documents\\\\report.pdf"
        ]),
        "FileSize": random.randint(1024, 10485760),
        "Hash": str(uuid.uuid4()).replace("-", ""),
        "Timestamp": (datetime.now() - timedelta(seconds=i)).isoformat()
    },
    # DNS query events
    lambda i: {
        "EventID": 22,
        "ProcessId": random.randint(1000, 10000),
        "QueryName": random.choice([
            "google.com",
            "microsoft.com",
            "suspicious-domain.com",
            "malware-c2.net",
            f"random-{uuid.uuid4().hex[:8]}.com"
        ]),
        "QueryType": random.choice(["A", "AAAA", "MX", "TXT"]),
        "QueryResult": f"{random.randint(1, 223)}.{random.randint(0, 255)}.{random.randint(0, 255)}.{random.randint(1, 254)}",
        "Timestamp": (datetime.now() - timedelta(seconds=i)).isoformat()
    }
]

# Generate events
for i in range($NUM_EVENTS):
    template = random.choice(event_templates)
    event = template(i)
    print(json.dumps(event))
EOF

EVENT_SIZE=$(du -h "$EVENT_FILE" | cut -f1)
echo -e "${GREEN}Generated $NUM_EVENTS events ($EVENT_SIZE)${NC}"
echo ""

# Build sigma-rs in release mode
echo -e "${BLUE}Building sigma-rs in release mode...${NC}"
cargo build --release --no-default-features 2>/dev/null || {
    echo -e "${RED}Build failed!${NC}"
    exit 1
}

# Run performance tests
echo -e "${BLUE}Running performance tests...${NC}"
echo ""

# Test 1: Throughput test
echo -e "${YELLOW}Test 1: Throughput Test${NC}"
START_TIME=$(date +%s.%N)
PROCESSED=$(./target/release/sigma-rs --rules "$RULES_DIR" < "$EVENT_FILE" 2>&1 | grep -c "rule_id")
END_TIME=$(date +%s.%N)
DURATION=$(echo "$END_TIME - $START_TIME" | bc)
EVENTS_PER_SEC=$(echo "scale=2; $NUM_EVENTS / $DURATION" | bc)
MATCHES_PER_SEC=$(echo "scale=2; $PROCESSED / $DURATION" | bc)

echo -e "Duration: ${GREEN}${DURATION}s${NC}"
echo -e "Events processed: ${GREEN}$NUM_EVENTS${NC}"
echo -e "Matches found: ${GREEN}$PROCESSED${NC}"
echo -e "Events/second: ${GREEN}$EVENTS_PER_SEC${NC}"
echo -e "Matches/second: ${GREEN}$MATCHES_PER_SEC${NC}"
echo ""

# Test 2: Memory usage test
echo -e "${YELLOW}Test 2: Memory Usage Test${NC}"
/usr/bin/time -l ./target/release/sigma-rs --rules "$RULES_DIR" < "$EVENT_FILE" > /dev/null 2> "$OUTPUT_DIR/memory_${TIMESTAMP}.txt" || true

if [ -f "$OUTPUT_DIR/memory_${TIMESTAMP}.txt" ]; then
    MAX_RSS=$(grep "maximum resident set size" "$OUTPUT_DIR/memory_${TIMESTAMP}.txt" | awk '{print $1}')
    if [ -n "$MAX_RSS" ]; then
        MAX_RSS_MB=$(echo "scale=2; $MAX_RSS / 1048576" | bc)
        echo -e "Peak memory usage: ${GREEN}${MAX_RSS_MB} MB${NC}"
    fi
fi
echo ""

# Test 3: Batch processing test
echo -e "${YELLOW}Test 3: Batch Processing Test${NC}"
BATCH_FILE="$OUTPUT_DIR/batch_${TIMESTAMP}.json"
TOTAL_TIME=0
BATCH_COUNT=$((NUM_EVENTS / BATCH_SIZE))

for i in $(seq 1 $BATCH_COUNT); do
    START=$((($i - 1) * BATCH_SIZE + 1))
    END=$(($i * BATCH_SIZE))
    
    sed -n "${START},${END}p" "$EVENT_FILE" > "$BATCH_FILE"
    
    BATCH_START=$(date +%s.%N)
    ./target/release/sigma-rs --rules "$RULES_DIR" < "$BATCH_FILE" > /dev/null 2>&1
    BATCH_END=$(date +%s.%N)
    
    BATCH_TIME=$(echo "$BATCH_END - $BATCH_START" | bc)
    TOTAL_TIME=$(echo "$TOTAL_TIME + $BATCH_TIME" | bc)
    
    printf "\rBatch %d/%d completed (%.3fs)" $i $BATCH_COUNT $BATCH_TIME
done
echo ""

AVG_BATCH_TIME=$(echo "scale=3; $TOTAL_TIME / $BATCH_COUNT" | bc)
echo -e "Average batch time: ${GREEN}${AVG_BATCH_TIME}s${NC}"
echo ""

# Test 4: Concurrent processing simulation
echo -e "${YELLOW}Test 4: Concurrent Processing Test${NC}"
CONCURRENT_PROCS=4
CONCURRENT_START=$(date +%s.%N)

for i in $(seq 1 $CONCURRENT_PROCS); do
    (
        PROC_FILE="$OUTPUT_DIR/proc_${i}_${TIMESTAMP}.json"
        PROC_START=$((($i - 1) * NUM_EVENTS / CONCURRENT_PROCS + 1))
        PROC_END=$(($i * NUM_EVENTS / CONCURRENT_PROCS))
        
        sed -n "${PROC_START},${PROC_END}p" "$EVENT_FILE" > "$PROC_FILE"
        ./target/release/sigma-rs --rules "$RULES_DIR" < "$PROC_FILE" > /dev/null 2>&1
    ) &
done

wait
CONCURRENT_END=$(date +%s.%N)
CONCURRENT_DURATION=$(echo "$CONCURRENT_END - $CONCURRENT_START" | bc)
CONCURRENT_EVENTS_PER_SEC=$(echo "scale=2; $NUM_EVENTS / $CONCURRENT_DURATION" | bc)

echo -e "Concurrent processes: ${GREEN}$CONCURRENT_PROCS${NC}"
echo -e "Total duration: ${GREEN}${CONCURRENT_DURATION}s${NC}"
echo -e "Events/second (concurrent): ${GREEN}$CONCURRENT_EVENTS_PER_SEC${NC}"
echo ""

# Generate summary report
REPORT_FILE="$OUTPUT_DIR/report_${TIMESTAMP}.txt"
cat > "$REPORT_FILE" << EOF
Sigma-rs Load Test Report
========================
Timestamp: $(date)
Events: $NUM_EVENTS
Rules: $RULE_COUNT

Throughput Test:
- Duration: ${DURATION}s
- Events/second: $EVENTS_PER_SEC
- Matches found: $PROCESSED

Memory Usage:
- Peak RSS: ${MAX_RSS_MB:-N/A} MB

Batch Processing:
- Batch size: $BATCH_SIZE
- Avg batch time: ${AVG_BATCH_TIME}s

Concurrent Processing:
- Processes: $CONCURRENT_PROCS
- Duration: ${CONCURRENT_DURATION}s
- Events/second: $CONCURRENT_EVENTS_PER_SEC
EOF

echo -e "${GREEN}=== Test Complete ===${NC}"
echo -e "Results saved to: ${BLUE}$REPORT_FILE${NC}"
echo ""

# Cleanup temporary files
rm -f "$BATCH_FILE" "$OUTPUT_DIR/proc_"*"_${TIMESTAMP}.json" 2>/dev/null

# Show summary
echo -e "${BLUE}Performance Summary:${NC}"
echo -e "├─ Single-threaded: ${GREEN}$EVENTS_PER_SEC events/sec${NC}"
echo -e "├─ Concurrent ($CONCURRENT_PROCS procs): ${GREEN}$CONCURRENT_EVENTS_PER_SEC events/sec${NC}"
echo -e "├─ Memory usage: ${GREEN}${MAX_RSS_MB:-N/A} MB${NC}"
echo -e "└─ Avg batch latency: ${GREEN}${AVG_BATCH_TIME}s${NC}"