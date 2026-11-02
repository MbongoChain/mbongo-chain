# Mbongo Chain - Makefile
# Convenient commands for development

.PHONY: help check lint test build clean fmt clippy audit docs dev-setup devnet-up devnet-down

help: ## Show this help message
	@echo "Mbongo Chain - Development Commands"
	@echo ""
	@grep -E '^[a-zA-Z_-]+:.*?## .*$$' $(MAKEFILE_LIST) | sort | awk 'BEGIN {FS = ":.*?## "}; {printf "\033[36m%-20s\033[0m %s\n", $$1, $$2}'

check: ## Run all pre-commit checks
	@./scripts/check.sh

lint: fmt clippy ## Format and lint code

fmt: ## Format code
	@cargo fmt --all

clippy: ## Run Clippy
	@cargo clippy --workspace --all-targets -- -D warnings

test: ## Run test suite
	@cargo test --workspace --verbose

test-coverage: ## Run tests with coverage
	@cargo tarpaulin --workspace --out Html --output-dir coverage

build: ## Build workspace (debug)
	@cargo build --workspace

build-release: ## Build workspace (release)
	@cargo build --workspace --release

clean: ## Clean build artifacts
	@cargo clean

audit: ## Run security audit
	@cargo audit

docs: ## Build documentation
	@cargo doc --workspace --no-deps --open

dev-setup: ## Set up development environment
	@./scripts/dev-setup.sh

ci: check test build ## Run CI checks locally


# --- Dockerised devnet (issue #53) -------------------------------------
# Both targets delegate to one script, so the local path and the CI path
# run exactly the same bootstrap and cannot drift apart.

devnet-up: ## Boot the deterministic 3-node Docker devnet and verify convergence
	@./scripts/devnet/docker-devnet.sh up

devnet-down: ## Stop the Docker devnet and remove its containers and network
	@./scripts/devnet/docker-devnet.sh down
