#!/bin/bash
# Simple script to run sigma-rs

# Colors for output
GREEN='\033[0;32m'
RED='\033[0;31m'
NC='\033[0m' # No Color

# Default values
RULES_DIR="./rules"
BUILD_MODE="release"
DEBUG=""

# Parse arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        --debug)
            DEBUG="--debug"
            BUILD_MODE="debug"
            shift
            ;;
        --rules)
            RULES_DIR="$2"
            shift 2
            ;;
        --help|-h)
            echo "Usage: ./run.sh [OPTIONS]"
            echo ""
            echo "Options:"
            echo "  --rules <dir>    Path to rules directory (default: ./rules)"
            echo "  --debug          Run in debug mode with verbose logging"
            echo "  --help           Show this help message"
            echo ""
            echo "Examples:"
            echo "  # Process events from file:"
            echo "  ./run.sh --rules ./my-rules < events.json"
            echo ""
            echo "  # Process events from stdin:"
            echo "  echo '{\"EventID\": 4688, \"Image\": \"cmd.exe\"}' | ./run.sh"
            echo ""
            echo "  # Debug mode:"
            echo "  ./run.sh --debug --rules ./rules < events.json"
            exit 0
            ;;
        *)
            echo "Unknown option: $1"
            exit 1
            ;;
    esac
done

# Check if rules directory exists
if [ ! -d "$RULES_DIR" ]; then
    echo -e "${RED}Error: Rules directory not found: $RULES_DIR${NC}"
    echo "Please specify a valid rules directory with --rules"
    exit 1
fi

# Build if needed
echo -e "${GREEN}Building sigma-rs in $BUILD_MODE mode...${NC}"
if [ "$BUILD_MODE" = "release" ]; then
    cargo build --release --no-default-features 2>/dev/null || {
        echo -e "${RED}Build failed!${NC}"
        exit 1
    }
    BINARY="./target/release/sigma-rs"
else
    cargo build --no-default-features 2>/dev/null || {
        echo -e "${RED}Build failed!${NC}"
        exit 1
    }
    BINARY="./target/debug/sigma-rs"
fi

# Run sigma-rs
echo -e "${GREEN}Running sigma-rs with rules from: $RULES_DIR${NC}"
echo -e "${GREEN}Reading events from stdin...${NC}"
$BINARY --rules "$RULES_DIR" $DEBUG