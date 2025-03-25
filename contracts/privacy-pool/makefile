
build:
	stellar contract build

build-no-utxo-events:
	stellar contract build --features no-utxo-events

# Run test with UTXO events disabled
test-no-utxo-events:
	cargo test --features no-utxo-events

# Default test run with no flags enabled
test-default:
	cargo test

test: test-no-utxo-events test-default

help:
	@echo "Available targets:"
	@echo "  build                - Build the contract"
	@echo "  build-no-utxo-events - Build the contract with no-utxo-events feature"
	@echo "  test-no-utxo-events  - Run tests with no-utxo-events feature"
	@echo "  test-default         - Run tests with default configuration"
	@echo "  test                 - Run both test-no-utxo-events and test-default"
	@echo "  help, h              - Show this help message"

h: help
