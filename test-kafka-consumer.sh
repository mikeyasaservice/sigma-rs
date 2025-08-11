#!/bin/bash
# Quick test script to verify Kafka consumer functionality

echo "Testing Kafka consumer pipeline..."
echo ""

# Send a test event to Kafka
TEST_EVENT='{"EventID": 1, "Image": "C:\\Windows\\System32\\cmd.exe", "CommandLine": "cmd.exe /c whoami"}'
echo "Sending test event: $TEST_EVENT"
echo "$TEST_EVENT" | docker exec -i redpanda-0 rpk topic produce security-events

echo ""
echo "Event sent to security-events topic. To consume and process:"
echo "  cargo run --features kafka --release -- --rules ./rules --input kafka --config config.toml"
echo ""
echo "Or check the topic manually:"
echo "  docker exec redpanda-0 rpk topic consume security-events --num 1"