.PHONY: build run test clean release help

# Default rule directory
RULES_DIR ?= ./rules

help:
	@echo "Sigma-rs Makefile"
	@echo ""
	@echo "Usage:"
	@echo "  make build       - Build sigma-rs in debug mode"
	@echo "  make release     - Build sigma-rs in release mode"
	@echo "  make run         - Build and run sigma-rs (reads from stdin)"
	@echo "  make test        - Run all tests"
	@echo "  make clean       - Clean build artifacts"
	@echo "  make help        - Show this help message"
	@echo ""
	@echo "Environment variables:"
	@echo "  RULES_DIR        - Path to rules directory (default: ./rules)"
	@echo ""
	@echo "Examples:"
	@echo "  make run < events.json"
	@echo "  RULES_DIR=/my/rules make run < events.json"

build:
	cargo build --no-default-features

release:
	cargo build --release --no-default-features

run: build
	@if [ ! -d "$(RULES_DIR)" ]; then \
		echo "Error: Rules directory not found: $(RULES_DIR)"; \
		echo "Set RULES_DIR environment variable or create ./rules directory"; \
		exit 1; \
	fi
	./target/debug/sigma-rs --rules $(RULES_DIR)

run-release: release
	@if [ ! -d "$(RULES_DIR)" ]; then \
		echo "Error: Rules directory not found: $(RULES_DIR)"; \
		echo "Set RULES_DIR environment variable or create ./rules directory"; \
		exit 1; \
	fi
	./target/release/sigma-rs --rules $(RULES_DIR)

test:
	cargo test --no-default-features

clean:
	cargo clean