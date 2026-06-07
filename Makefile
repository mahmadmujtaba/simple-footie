.PHONY: all build test server tui check clean release help

# ── Build ────────────────────────────────────────────────────────

build: ## Build all crates in release mode
	cargo build --release

check: ## Check compilation (fast)
	cargo check

clean: ## Clean build artifacts
	cargo clean

release: build ## Alias for build

# ── Run ──────────────────────────────────────────────────────────

server: ## Run the game server (UDP on port 9001)
	cargo run --release -p server

tui: ## Run the TUI client
	cargo run --release -p tui

# ── Test ─────────────────────────────────────────────────────────

test: ## Run all tests
	cargo test

test-quick: ## Run tests without debug deps (faster)
	cargo test --release

# ── Lint ─────────────────────────────────────────────────────────

lint: ## Run clippy
	cargo clippy --all-targets -- -D warnings

fmt: ## Format code
	cargo fmt

# ── Utility ──────────────────────────────────────────────────────

docs: ## Open rustdoc for all crates
	cargo doc --open --no-deps

outdated: ## Check for outdated dependencies
	cargo outdated 2>/dev/null || echo "Install with: cargo install cargo-outdated"

size: ## Show binary sizes
	ls -lh target/release/server target/release/tui 2>/dev/null || echo "Build first with: make build"

# ── Help ─────────────────────────────────────────────────────────

help: ## Show this help
	@grep -E '^[a-zA-Z_-]+:.*?## ' Makefile | sort | \
		awk 'BEGIN {FS = ":.*?## "}; {printf "\033[36m%-20s\033[0m %s\n", $$1, $$2}'
