.PHONY: dev build test lint fmt clean setup

# Development
dev:                ## Start daemon in dev mode
	cargo run -p homerund

tui:                ## Start TUI (daemon must be running)
	cargo run -p homerun

desktop:            ## Start Tauri desktop app (daemon must be running)
	cd apps/desktop && npm run tauri dev

# Build
build:              ## Build daemon + TUI (release)
	cargo build --release -p homerund -p homerun

build-all:          ## Build everything including desktop app
	cargo build --release -p homerund -p homerun
	cd apps/desktop && npm install && npm run build

# Test
test:               ## Run all tests (Rust + React)
	cargo test
	cd apps/desktop && npm test

test-rust:          ## Run Rust tests only
	cargo test

test-react:         ## Run React tests only
	cd apps/desktop && npm test

test-coverage:      ## Run all tests with coverage
	./scripts/test-all.sh

# Code quality
lint:               ## Run all linters
	cargo clippy --all-targets --all-features -- -D warnings
	cd apps/desktop && npx tsc --noEmit

fmt:                ## Format all code
	cargo fmt
	cd apps/desktop && npx prettier --write src/

fmt-check:          ## Check formatting without changes
	cargo fmt --check
	cd apps/desktop && npx prettier --check src/

# Setup
setup:              ## First-time setup
	./scripts/setup.sh

clean:              ## Clean build artifacts
	cargo clean
	rm -rf apps/desktop/dist apps/desktop/src-tauri/target

# Help
help:               ## Show this help
	@grep -E '^[a-zA-Z_-]+:.*?## .*$$' $(MAKEFILE_LIST) | sort | awk 'BEGIN {FS = ":.*?## "}; {printf "\033[36m%-16s\033[0m %s\n", $$1, $$2}'

.DEFAULT_GOAL := help
