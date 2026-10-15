# Common tasks for mhtml. Run `make` (or `make help`) to list them.

.DEFAULT_GOAL := help
# Chrome/Chromium used by `make screenshots` (override for other systems)
CHROME ?= /Applications/Google Chrome.app/Contents/MacOS/Google Chrome

.PHONY: help build release run preview example screenshots test test-rust test-npm fmt fmt-check lint check \
        install uninstall npm-pack bump version version-check tag clean

CARGO ?= cargo
NPM   ?= npm

# Arguments passed to the binary by `make run`, e.g. make run ARGS="examples/example.md out.html --theme dark"
ARGS ?= examples/example.md --no-open

CARGO_VERSION := $(shell sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -n1)
NPM_VERSION   := $(shell node -p "require('./npm/package.json').version" 2>/dev/null)

help: ## Show this help
	@awk 'BEGIN {FS = ":.*## "} /^[a-zA-Z_-]+:.*## / {printf "  \033[36m%-14s\033[0m %s\n", $$1, $$2}' $(MAKEFILE_LIST)

build: ## Debug build
	$(CARGO) build

release: ## Optimized release build (target/release/mhtml)
	$(CARGO) build --release

run: ## Run the CLI: make run ARGS="a.md out.html --theme dark"
	$(CARGO) run -- $(ARGS)

preview: ## Preview examples/example.md in the browser
	$(CARGO) run -- examples/example.md --toc

example: ## Render examples/example.md to target/example.html
	@mkdir -p target
	$(CARGO) run -q -- --no-config examples/example.md target/example.html --toc

screenshots: release ## Regenerate docs/screenshot-{light,dark}.png (needs Chrome; set CHROME=...)
	@mkdir -p target/shots
	@for t in light dark; do \
		./target/release/mhtml --no-config examples/screenshot.md target/shots/$$t.html --theme $$t --lang zh-TW && \
		"$(CHROME)" --headless=new --disable-gpu --hide-scrollbars --force-device-scale-factor=2 \
			--window-size=860,665 --screenshot=docs/screenshot-$$t.png "file://$(CURDIR)/target/shots/$$t.html"; \
	done

test: test-rust test-npm test-scripts ## Run all tests

test-rust: ## Rust unit and integration tests
	$(CARGO) test --all-targets

test-npm: ## npm wrapper tests
	cd npm && $(NPM) test

test-scripts: ## Release script tests
	node --test scripts/*.test.js

fmt: ## Format the code
	$(CARGO) fmt

fmt-check: ## Check formatting (as CI does)
	$(CARGO) fmt --check

lint: ## Clippy with warnings as errors (as CI does)
	$(CARGO) clippy --all-targets -- -D warnings

check: fmt-check lint test ## Everything CI runs

install: ## Install mhtml from this checkout into ~/.cargo/bin
	$(CARGO) install --path . --locked

uninstall: ## Remove the cargo-installed mhtml
	$(CARGO) uninstall mhtml

npm-pack: ## Dry-run the npm package contents
	cp LICENSE README.md npm/
	cd npm && $(NPM) pack --dry-run; status=$$?; rm -f LICENSE README.md; exit $$status

bump: ## Bump the version in Cargo.toml, npm/package.json and Cargo.lock: make bump LEVEL=patch|minor|major
	@test -n "$(LEVEL)" || { echo "usage: make bump LEVEL=patch|minor|major"; exit 1; }
	node scripts/bump-version.js $(LEVEL)
	$(CARGO) check --quiet
	@$(MAKE) --no-print-directory version-check

version: ## Show the Cargo and npm versions
	@echo "Cargo.toml:        $(CARGO_VERSION)"
	@echo "npm/package.json:  $(NPM_VERSION)"

version-check: ## Fail unless Cargo.toml and npm/package.json agree
	@if [ "$(CARGO_VERSION)" != "$(NPM_VERSION)" ]; then \
		echo "Version mismatch: Cargo.toml=$(CARGO_VERSION) npm/package.json=$(NPM_VERSION)"; exit 1; \
	fi
	@echo "Versions match: $(CARGO_VERSION)"

tag: check version-check ## Create the local release tag vX.Y.Z (push it yourself to release)
	@test -z "$$(git status --porcelain)" || { echo "Working tree is not clean"; exit 1; }
	git tag -a "v$(CARGO_VERSION)" -m "Release v$(CARGO_VERSION)"
	@echo "Created v$(CARGO_VERSION). Publish with: git push origin v$(CARGO_VERSION)"

clean: ## Remove build artifacts
	$(CARGO) clean
	rm -rf npm/vendor
