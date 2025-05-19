#!/bin/bash

# Test runner script for Sigma-rs

set -e

echo "🧪 Running Sigma-rs Test Suite"
echo "============================="

# Set test environment
export RUST_BACKTRACE=1
export RUST_LOG=sigma_rs=debug

# Run unit tests
echo -e "\n📋 Running unit tests..."
cargo test --lib

# Run doc tests
echo -e "\n📚 Running documentation tests..."
cargo test --doc

# Run integration tests
echo -e "\n🔗 Running integration tests..."
cargo test --test '*' -- --test-threads=1

# Run consumer tests (require Docker)
if command -v docker &> /dev/null; then
    echo -e "\n🐳 Running consumer integration tests..."
    cargo test --test consumer_integration_test -- --test-threads=1
else
    echo -e "\n⚠️  Skipping consumer tests (Docker not available)"
fi

# Run benchmarks (quick mode)
echo -e "\n⚡ Running benchmarks (quick)..."
cargo bench -- --quick

# Run with coverage if requested
if [[ "$1" == "--coverage" ]]; then
    echo -e "\n📊 Running with coverage..."
    ./scripts/coverage.sh
fi

# Check for clippy warnings
echo -e "\n🔍 Running clippy..."
cargo clippy -- -D warnings || echo "⚠️  Clippy warnings found"

# Format check
echo -e "\n✨ Checking formatting..."
cargo fmt -- --check || echo "⚠️  Formatting issues found"

echo -e "\n✅ Test suite complete!"

# Summary
echo -e "\nTest Summary:"
echo "============="
cargo test --quiet 2>&1 | grep -E "(test result:|passed|failed)" || true