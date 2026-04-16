PREFIX ?= /usr/local
SHARE_DIR = $(PREFIX)/share/icloud-nfs-exporter

.PHONY: build build-release test lint clean install uninstall help

help: ## Show available targets
	@grep -E '^[a-zA-Z_-]+:.*##' $(MAKEFILE_LIST) | sort | \
		awk 'BEGIN {FS = ":.*##"}; {printf "  \033[36m%-16s\033[0m %s\n", $$1, $$2}'

# ── Build ──

build: ## Debug build (Swift + Rust)
	swift build --package-path src/hydration
	swift build --package-path src/app
	swift build --package-path src/helper
	cd src/fuse && cargo build

build-release: ## Optimised release build
	swift build --package-path src/hydration -c release
	swift build --package-path src/app -c release
	swift build --package-path src/helper -c release
	cd src/fuse && cargo build --release

# ── Test ──

test: ## Run all tests (Swift + Rust + Python)
	swift test --package-path src/hydration
	cd src/fuse && cargo test
	python3 -m unittest discover -s tests -v

# ── Lint ──

lint: ## Lint all languages
	swiftlint lint src/
	cd src/fuse && cargo clippy -- -D warnings
	ruff check scripts/

# ── Install / Uninstall ──

install: build-release ## Build and install to PREFIX (/usr/local)
	install -d $(PREFIX)/bin
	install -d $(SHARE_DIR)/scripts/icne_lib
	install -d $(SHARE_DIR)/launchd
	install -m 755 src/hydration/.build/release/HydrationDaemon $(PREFIX)/bin/
	install -m 755 src/app/.build/release/MenuBarApp $(PREFIX)/bin/icloud-nfs-exporter-app
	install -m 755 scripts/icne $(SHARE_DIR)/scripts/icne
	install -m 644 scripts/icne_lib/*.py $(SHARE_DIR)/scripts/icne_lib/
	install -m 644 launchd/*.template $(SHARE_DIR)/launchd/
	ln -sf $(SHARE_DIR)/scripts/icne $(PREFIX)/bin/icne
	@echo "Installed to $(PREFIX). Run 'icne setup' to configure."

uninstall: ## Remove installed files
	rm -f $(PREFIX)/bin/HydrationDaemon
	rm -f $(PREFIX)/bin/icloud-nfs-exporter-app
	rm -f $(PREFIX)/bin/icne
	rm -rf $(SHARE_DIR)
	@echo "Uninstalled. LaunchAgent and config are preserved."

# ── Clean ──

clean: ## Remove build artifacts
	swift package --package-path src/hydration clean
	swift package --package-path src/app clean
	swift package --package-path src/helper clean
	cd src/fuse && cargo clean
