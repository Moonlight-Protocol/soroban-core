
build:
	stellar contract build

# Run only the macro test with UTXO events disabled
test-no-utxo-events:
	cargo test test_mint_event_controlled_by_macro --features no-utxo-events

# Default test run with no flags enabled
test-default:
	cargo test

test: test-no-utxo-events test-default

help:
	@echo "Available targets:"
	@echo "  build                - Build the contract"
	@echo "  test-no-utxo-events  - Run tests with no-utxo-events feature"
	@echo "  test-default         - Run tests with default configuration"
	@echo "  test                 - Run both test-no-utxo-events and test-default"
	@echo "  help, h              - Show this help message"

h: help
