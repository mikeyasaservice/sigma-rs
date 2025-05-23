#!/bin/bash

# Test script for Sigma-rs examples

echo "Testing Sigma-rs examples..."
echo

# Colors for output
GREEN='\033[0;32m'
RED='\033[0;31m'
NC='\033[0m' # No Color

# Test rule validator
echo "1. Testing rule_validator..."
cargo run --example rule_validator -- --rule-dirs examples/data/rules -v
if [ $? -eq 0 ]; then
    echo -e "${GREEN}✓ rule_validator passed${NC}"
else
    echo -e "${RED}✗ rule_validator failed${NC}"
fi
echo

# Test event detector
echo "2. Testing event_detector..."
cargo run --example event_detector -- --rule-dirs examples/data/rules --events examples/data/test_events.json --stdout
if [ $? -eq 0 ]; then
    echo -e "${GREEN}✓ event_detector passed${NC}"
else
    echo -e "${RED}✗ event_detector failed${NC}"
fi
echo

# Test stream detector
echo "3. Testing stream_detector..."
cat examples/data/test_events.json | jq -c '.[]' | cargo run --example stream_detector -- --rule-dirs examples/data/rules
if [ $? -eq 0 ]; then
    echo -e "${GREEN}✓ stream_detector passed${NC}"
else
    echo -e "${RED}✗ stream_detector failed${NC}"
fi
echo

# Test parallel stream detector
echo "4. Testing parallel_stream_detector..."
cat examples/data/test_events.json | jq -c '.[]' | cargo run --example parallel_stream_detector -- --rule-dirs examples/data/rules --workers 2
if [ $? -eq 0 ]; then
    echo -e "${GREEN}✓ parallel_stream_detector passed${NC}"
else
    echo -e "${RED}✗ parallel_stream_detector failed${NC}"
fi
echo

echo "All example tests completed!"