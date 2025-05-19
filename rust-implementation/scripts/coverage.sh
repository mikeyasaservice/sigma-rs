#!/bin/bash

# Install cargo-tarpaulin if not already installed
if ! command -v cargo-tarpaulin &> /dev/null
then
    echo "Installing cargo-tarpaulin..."
    cargo install cargo-tarpaulin
fi

# Run tests with coverage
echo "Running tests with coverage..."
cargo tarpaulin --config .tarpaulin.toml --verbose

# Check if coverage meets threshold
COVERAGE=$(cargo tarpaulin --print-summary 2>/dev/null | grep "Coverage" | awk '{print $2}' | sed 's/%//')
echo "Coverage: $COVERAGE%"

# Check if coverage meets 85% threshold
if (( $(echo "$COVERAGE < 85" | bc -l) )); then
    echo "ERROR: Coverage $COVERAGE% is below 85% threshold"
    exit 1
else
    echo "SUCCESS: Coverage $COVERAGE% meets 85% threshold"
fi

# Open coverage report in browser (optional)
if [[ "$1" == "--open" ]]; then
    open target/coverage/tarpaulin-report.html
fi
