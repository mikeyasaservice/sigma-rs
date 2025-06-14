#!/bin/bash
# Enhanced script to run sigma-rs with input/output options

# Colors for output
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Default values
RULES_DIR="./rules"
BUILD_MODE="release"
DEBUG=""
INPUT="stdin"
OUTPUT="stdout"
CONFIG=""
KAFKA_FEATURE=""

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
        --input)
            INPUT="$2"
            shift 2
            ;;
        --output)
            OUTPUT="$2"
            shift 2
            ;;
        --config)
            CONFIG="--config $2"
            shift 2
            ;;
        --help|-h)
            echo "Usage: ./run.sh [OPTIONS]"
            echo ""
            echo "Options:"
            echo "  --rules <dir>     Path to rules directory (default: ./rules)"
            echo "  --input <source>  Input source: stdin or kafka (default: stdin)"
            echo "  --output <target> Output target: stdout or kafka (default: stdout)"
            echo "  --config <file>   Configuration file (required for kafka)"
            echo "  --debug           Run in debug mode with verbose logging"
            echo "  --help            Show this help message"
            echo ""
            echo "Examples:"
            echo "  # Process events from file to stdout:"
            echo "  ./run.sh --rules ./my-rules < events.json"
            echo ""
            echo "  # Process events from Kafka to stdout:"
            echo "  ./run.sh --input kafka --config config.toml"
            echo ""
            echo "  # Process events from stdin to Kafka:"
            echo "  ./run.sh --output kafka --config config.toml < events.json"
            echo ""
            echo "  # Process events from Kafka to Kafka:"
            echo "  ./run.sh --input kafka --output kafka --config config.toml"
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

# Validate input/output options
if [[ "$INPUT" != "stdin" && "$INPUT" != "kafka" ]]; then
    echo -e "${RED}Error: Invalid input source: $INPUT${NC}"
    echo "Input must be 'stdin' or 'kafka'"
    exit 1
fi

if [[ "$OUTPUT" != "stdout" && "$OUTPUT" != "kafka" ]]; then
    echo -e "${RED}Error: Invalid output target: $OUTPUT${NC}"
    echo "Output must be 'stdout' or 'kafka'"
    exit 1
fi

# Check if Kafka is needed
if [[ "$INPUT" == "kafka" || "$OUTPUT" == "kafka" ]]; then
    KAFKA_FEATURE="--features kafka"
    
    # Check if config file is provided
    if [[ -z "$CONFIG" ]]; then
        echo -e "${RED}Error: Configuration file required for Kafka input/output${NC}"
        echo "Use --config <file> to specify configuration"
        echo ""
        echo "Example config.toml:"
        echo "[kafka]"
        echo "brokers = \"localhost:9092\""
        echo "input_topic = \"security-events\""
        echo "output_topic = \"sigma-matches\""
        echo "group_id = \"sigma-rs\""
        exit 1
    fi
fi

# Build if needed
echo -e "${GREEN}Building sigma-rs in $BUILD_MODE mode...${NC}"
if [ "$BUILD_MODE" = "release" ]; then
    cargo build --release --no-default-features $KAFKA_FEATURE 2>/dev/null || {
        echo -e "${RED}Build failed!${NC}"
        echo "If building with Kafka, ensure rdkafka dependencies are installed"
        exit 1
    }
    BINARY="./target/release/sigma-rs"
else
    cargo build --no-default-features $KAFKA_FEATURE 2>/dev/null || {
        echo -e "${RED}Build failed!${NC}"
        echo "If building with Kafka, ensure rdkafka dependencies are installed"
        exit 1
    }
    BINARY="./target/debug/sigma-rs"
fi

# Display configuration
echo -e "${GREEN}Configuration:${NC}"
echo -e "  Rules: $RULES_DIR"
echo -e "  Input: $INPUT"
echo -e "  Output: $OUTPUT"
if [[ -n "$CONFIG" ]]; then
    echo -e "  Config: ${CONFIG#--config }"
fi

# Run sigma-rs
if [[ "$INPUT" == "stdin" ]]; then
    echo -e "${YELLOW}Reading events from stdin...${NC}"
fi

$BINARY --rules "$RULES_DIR" --input "$INPUT" --output "$OUTPUT" $CONFIG $DEBUG