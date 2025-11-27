# configuration parameters

CHANNEL = privacy_channel
AUTH = channel_auth_contract
AUTH_ID = CCPJDISZOC3VSCNB7VCWSFQHAMH7GI7PJBSPDNAJCZBYCPPLSMRIS2P7
ASSET = CDLZFC3SYJYDZT7K67VZ75HPJVIEUVNIXF47ZG2FB2RMQQVU2HHGCYSC
NETWORK = testnet
SOURCE_ACCOUNT = admin
WASM_PATH = target/wasm32v1-none/release/
BINDINGS_DIR = ./.bindings

# scripts

CYAN = [36m
GREEN = [32m
YELLOW = [33m
BLUE = [34m
RESET = [0m

help h: 
	@echo "$(CYAN)make build$(RESET)      $(YELLOW)build the contract wasm$(RESET)"
	@echo "$(CYAN)make deploy$(RESET)     $(YELLOW)deploy the contract to $(NETWORK)$(RESET)"
	@echo "$(CYAN)make bindings$(RESET)   $(YELLOW)generate TypeScript bindings$(RESET)"
	@echo "$(CYAN)make clean$(RESET)      $(YELLOW)remove build artifacts$(RESET)"

build:
	stellar contract build

deploy-auth: 
	stellar contract deploy \
		--wasm $(WASM_PATH)$(AUTH).wasm \
		--network $(NETWORK) \
		--source-account $(SOURCE_ACCOUNT) \
		-- --admin $(SOURCE_ACCOUNT)

deploy-channel: 
	stellar contract deploy \
		--wasm $(WASM_PATH)$(CHANNEL).wasm \
		--network $(NETWORK) \
		--source-account $(SOURCE_ACCOUNT) \
		-- --admin $(SOURCE_ACCOUNT) --auth_contract $(AUTH_ID) --asset $(ASSET)

bindings-auth: 
	stellar contract bindings typescript \
		--wasm $(WASM_PATH)$(AUTH).wasm \
		--output-dir $(BINDINGS_DIR)/auth \
		--overwrite

bindings-channel: 
	stellar contract bindings typescript \
		--wasm $(WASM_PATH)$(CHANNEL).wasm \
		--output-dir $(BINDINGS_DIR)/channel \
		--overwrite

clean:
	cargo clean
	rm -rf $(BINDINGS_DIR)