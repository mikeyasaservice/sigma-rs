#!/bin/bash

# Compatibility testing script for Sigma rule engine
# This script runs tests to compare Go and Rust implementations

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
NC='\033[0m' # No Color

echo "=== Sigma Rule Engine Compatibility Testing ==="
echo

# Check if Go implementation exists
GO_IMPL="../"
if [ ! -d "$GO_IMPL" ]; then
    echo -e "${RED}Error: Go implementation not found at $GO_IMPL${NC}"
    exit 1
fi

# Build Go test binary
echo "Building Go test binary..."
cd "$GO_IMPL"
go build -o sigma-go-test/sigma-test ./examples/compatibility-test/
cd -

# Create test fixtures directory
mkdir -p tests/fixtures/compatibility

# Generate compatibility test cases
cat > tests/fixtures/compatibility/test_cases.json << 'EOF'
[
  {
    "id": "simple-match",
    "description": "Simple event matching",
    "rule": "title: Simple Rule\nid: test-001\ndetection:\n  selection:\n    EventID: 1\n    Image|endswith: '\\cmd.exe'\n  condition: selection",
    "event": {
      "EventID": 1,
      "Image": "C:\\Windows\\System32\\cmd.exe"
    },
    "expected_result": {
      "matched": true,
      "rule_id": "test-001",
      "tags": [],
      "level": "medium"
    }
  },
  {
    "id": "complex-condition",
    "description": "Complex condition with AND/OR",
    "rule": "title: Complex Rule\nid: test-002\nlevel: high\ndetection:\n  proc:\n    EventID: 1\n  net:\n    EventID: 3\n  susp:\n    CommandLine|contains: 'mimikatz'\n  condition: (proc or net) and susp",
    "event": {
      "EventID": 1,
      "CommandLine": "cmd.exe /c mimikatz.exe"
    },
    "expected_result": {
      "matched": true,
      "rule_id": "test-002",
      "tags": [],
      "level": "high"
    }
  },
  {
    "id": "array-values",
    "description": "Matching against array values",
    "rule": "title: Array Test\nid: test-003\ndetection:\n  selection:\n    EventID: [1, 4688]\n    Image:\n      - '*\\cmd.exe'\n      - '*\\powershell.exe'\n  condition: selection",
    "event": {
      "EventID": 4688,
      "Image": "C:\\Windows\\System32\\WindowsPowerShell\\v1.0\\powershell.exe"
    },
    "expected_result": {
      "matched": true,
      "rule_id": "test-003",
      "tags": [],
      "level": "medium"
    }
  },
  {
    "id": "modifiers",
    "description": "Testing various modifiers",
    "rule": "title: Modifier Test\nid: test-004\ndetection:\n  selection:\n    Path|contains: 'Windows'\n    Name|startswith: 'svc'\n    File|endswith: '.exe'\n  condition: selection",
    "event": {
      "Path": "C:\\Windows\\System32",
      "Name": "svchost",
      "File": "svchost.exe"
    },
    "expected_result": {
      "matched": true,
      "rule_id": "test-004",
      "tags": [],
      "level": "medium"
    }
  },
  {
    "id": "not-condition",
    "description": "Testing NOT operator",
    "rule": "title: NOT Test\nid: test-005\ndetection:\n  selection:\n    EventID: 1\n  filter:\n    User: 'SYSTEM'\n  condition: selection and not filter",
    "event": {
      "EventID": 1,
      "User": "john.doe"
    },
    "expected_result": {
      "matched": true,
      "rule_id": "test-005",
      "tags": [],
      "level": "medium"
    }
  }
]
EOF

# Run Rust tests
echo -e "\n${YELLOW}Running Rust tests...${NC}"
cargo test --test compatibility_tests -- --nocapture

# Run compatibility comparison
echo -e "\n${YELLOW}Running compatibility tests...${NC}"
cargo run --bin compatibility_runner

# Check results
if [ $? -eq 0 ]; then
    echo -e "\n${GREEN}✓ All compatibility tests passed!${NC}"
else
    echo -e "\n${RED}✗ Some compatibility tests failed${NC}"
    exit 1
fi

# Run benchmarks
echo -e "\n${YELLOW}Running performance benchmarks...${NC}"
cargo bench

# Generate coverage report
echo -e "\n${YELLOW}Generating coverage report...${NC}"
cargo tarpaulin --out Html --output-dir coverage

echo -e "\n${GREEN}Compatibility testing complete!${NC}"
echo "Coverage report available at: coverage/tarpaulin-report.html"