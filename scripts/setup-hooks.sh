#!/bin/bash

echo "Setting up pre-commit hooks..."

# Check if pre-commit is installed
if ! command -v pre-commit &> /dev/null; then
    echo "pre-commit not found. Installing..."
    if command -v pip &> /dev/null; then
        pip install pre-commit
    elif command -v pip3 &> /dev/null; then
        pip3 install pre-commit
    else
        echo "Error: pip not found. Please install pre-commit manually."
        exit 1
    fi
fi

# Install pre-commit hooks
pre-commit install
pre-commit install --hook-type commit-msg

# Check if cargo-audit is installed
if ! command -v cargo-audit &> /dev/null; then
    echo "cargo-audit not found. Installing..."
    cargo install cargo-audit
fi

echo "Pre-commit hooks installed successfully!"
echo "Run 'pre-commit run --all-files' to test all hooks"