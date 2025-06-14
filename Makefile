.PHONY: build run test clean release help kafka-build kafka-run

# Default rule directory
RULES_DIR ?= ./rules
CONFIG_FILE ?= config.toml
INPUT ?= stdin
OUTPUT ?= stdout

help:
	@echo "Sigma-rs Makefile"
	@echo ""
	@echo "Usage:"
	@echo "  make build         - Build sigma-rs in debug mode (no Kafka)"
	@echo "  make kafka-build   - Build sigma-rs with Kafka support"
	@echo "  make release       - Build sigma-rs in release mode (no Kafka)"
	@echo "  make kafka-release - Build sigma-rs in release mode with Kafka"
	@echo "  make run           - Run sigma-rs (stdin to stdout)"
	@echo "  make kafka-run     - Run sigma-rs with Kafka support"
	@echo "  make test          - Run all tests"
	@echo "  make clean         - Clean build artifacts"
	@echo "  make help          - Show this help message"
	@echo ""
	@echo "Environment variables:"
	@echo "  RULES_DIR    - Path to rules directory (default: ./rules)"
	@echo "  CONFIG_FILE  - Path to config file (default: config.toml)"
	@echo "  INPUT        - Input source: stdin or kafka (default: stdin)"
	@echo "  OUTPUT       - Output target: stdout or kafka (default: stdout)"
	@echo ""
	@echo "Examples:"
	@echo "  make run < events.json"
	@echo "  INPUT=kafka OUTPUT=stdout make kafka-run"
	@echo "  INPUT=kafka OUTPUT=kafka CONFIG_FILE=prod.toml make kafka-run"

build:
	cargo build --no-default-features

kafka-build:
	cargo build --features kafka

release:
	cargo build --release --no-default-features

kafka-release:
	cargo build --release --features kafka

run: build
	@if [ ! -d "$(RULES_DIR)" ]; then \
		echo "Error: Rules directory not found: $(RULES_DIR)"; \
		echo "Set RULES_DIR environment variable or create ./rules directory"; \
		exit 1; \
	fi
	./target/debug/sigma-rs --rules $(RULES_DIR) --input stdin --output stdout

kafka-run: kafka-build
	@if [ ! -d "$(RULES_DIR)" ]; then \
		echo "Error: Rules directory not found: $(RULES_DIR)"; \
		echo "Set RULES_DIR environment variable or create ./rules directory"; \
		exit 1; \
	fi
	@if [ "$(INPUT)" = "kafka" -o "$(OUTPUT)" = "kafka" ]; then \
		if [ ! -f "$(CONFIG_FILE)" ]; then \
			echo "Error: Configuration file not found: $(CONFIG_FILE)"; \
			echo "Set CONFIG_FILE environment variable or create config.toml"; \
			exit 1; \
		fi; \
		./target/debug/sigma-rs --rules $(RULES_DIR) --input $(INPUT) --output $(OUTPUT) --config $(CONFIG_FILE); \
	else \
		./target/debug/sigma-rs --rules $(RULES_DIR) --input $(INPUT) --output $(OUTPUT); \
	fi

test:
	cargo test --no-default-features

clean:
	cargo clean