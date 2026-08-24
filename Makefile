# GraphQL codegen helpers for regenerating Rust modules from an upstream Shopify CLI checkout.
#
# Usage:
#   make help
#   make codegen UPSTREAM_CLI=/path/to/shopify/cli
#   make codegen-check UPSTREAM_CLI=/path/to/shopify/cli

.DEFAULT_GOAL := help

UPSTREAM_CLI ?= ../gitCloned/cli
UPSTREAM_APP_GRAPHQL ?= $(UPSTREAM_CLI)/packages/app/src/cli/api/graphql
UPSTREAM_ADMIN_GRAPHQL ?= $(UPSTREAM_CLI)/packages/cli-kit/src/cli/api/graphql/admin
OUT_GRAPHQL ?= crates/cli-kit/src/api/generated/graphql

APP_SURFACES := app-management partners bulk-operations functions app-dev webhooks

CONSOLE_SRC ?= $(UPSTREAM_CLI)/packages/ui-extensions-dev-console
CONSOLE_OUT ?= crates/app/assets/dev-console
BRIDGE_OUT ?= target/bridge-release/bridge
BRIDGE_NODE_OUT ?= $(BRIDGE_OUT)/node-cli
BRIDGE_THEME_TOOLS_OUT ?= $(BRIDGE_OUT)/theme-tools
NODE_RUNTIME_DIR ?=
DIST_DIR ?= target/dist
DIST_PLATFORM ?= $(shell os=$$(uname -s | tr '[:upper:]' '[:lower:]'); arch=$$(uname -m); if echo "$$os" | grep -Eq '^(mingw|msys|cygwin)'; then os=win32; fi; if [ "$$arch" = x86_64 ]; then arch=x64; elif [ "$$arch" = aarch64 ] || [ "$$arch" = arm64 ]; then arch=arm64; elif [ "$$arch" = i386 ] || [ "$$arch" = i686 ]; then arch=ia32; fi; echo $$os-$$arch)
DIST_NAME ?= shopify-rust-$(DIST_PLATFORM)
RELEASE_ROOT ?= target/release-package/$(DIST_NAME)
RELEASE_ARCHIVE ?= $(DIST_DIR)/$(DIST_NAME).tar.gz
BRIDGE_ARCHIVE ?= $(DIST_DIR)/shopify-rust-bridge-$(DIST_PLATFORM).tar.gz

.PHONY: help theme-parity-check codegen-verify-upstream codegen-app codegen-admin codegen codegen-test codegen-check codegen-verify console-assets bridge-verify-upstream bridge-build-upstream bridge-stage bridge-stage-full bridge-archive bridge-size release-package release-smoke release-manifest

help:
	@echo "GraphQL codegen targets"
	@echo ""
	@echo "  make codegen              Regenerate app surfaces + admin modules"
	@echo "  make codegen-app          Regenerate app surfaces only"
	@echo "  make codegen-admin        Regenerate admin modules only"
	@echo "  make codegen-test         Run graphql-codegen unit tests"
	@echo "  make codegen-check        codegen + generator tests + cargo check -p cli-kit"
	@echo "  make codegen-verify       Fail if regenerating changes committed output"
	@echo "  make codegen-verify-upstream  Fail if upstream paths are missing"
	@echo "  make console-assets       Build ui-extensions-dev-console → crates/app/assets/dev-console"
	@echo "  make theme-parity-check   Verify the pinned upstream theme contract"
	@echo "  make bridge-stage         Stage minimal bundled Node bridge assets"
	@echo "  make bridge-stage-full    Stage full upstream bridge assets for debugging"
	@echo "  make bridge-archive       Create the verified bridge download archive"
	@echo "  make bridge-size          Show full/minimal bridge payload sizes"
	@echo "  make release-package      Build Rust release + bridge archive + manifest"
	@echo "  make release-smoke        Smoke test packaged release layout"
	@echo ""
	@echo "Variables (override on the command line):"
	@echo "  UPSTREAM_CLI=$(UPSTREAM_CLI)"
	@echo "  UPSTREAM_APP_GRAPHQL=$(UPSTREAM_APP_GRAPHQL)"
	@echo "  UPSTREAM_ADMIN_GRAPHQL=$(UPSTREAM_ADMIN_GRAPHQL)"
	@echo "  OUT_GRAPHQL=$(OUT_GRAPHQL)"
	@echo ""
	@echo "Example:"
	@echo "  make codegen UPSTREAM_CLI=/path/to/shopify/cli"
	@echo ""
	@echo "Upstream must already contain .graphql + generated/*.ts + types.d.ts"
	@echo "(run Shopify/cli's own GraphQL codegen first if those TS artifacts are stale)."

theme-parity-check:
	cargo build -p cli-kit --bin shopify
	python3 scripts/theme-parity-check.py "$(UPSTREAM_CLI)" target/debug/shopify

codegen-verify-upstream:
	@if [ ! -d "$(UPSTREAM_CLI)" ]; then \
		echo "error: UPSTREAM_CLI does not exist: $(UPSTREAM_CLI)"; \
		echo "hint: make codegen UPSTREAM_CLI=/path/to/shopify/cli"; \
		exit 1; \
	fi
	@if [ ! -d "$(UPSTREAM_APP_GRAPHQL)" ]; then \
		echo "error: UPSTREAM_APP_GRAPHQL does not exist: $(UPSTREAM_APP_GRAPHQL)"; \
		exit 1; \
	fi
	@if [ ! -d "$(UPSTREAM_ADMIN_GRAPHQL)" ]; then \
		echo "error: UPSTREAM_ADMIN_GRAPHQL does not exist: $(UPSTREAM_ADMIN_GRAPHQL)"; \
		exit 1; \
	fi
	@missing=0; \
	for surface in $(APP_SURFACES); do \
		if [ ! -d "$(UPSTREAM_APP_GRAPHQL)/$$surface" ]; then \
			echo "error: missing app surface dir: $(UPSTREAM_APP_GRAPHQL)/$$surface"; \
			missing=1; \
		fi; \
	done; \
	if [ "$$missing" -ne 0 ]; then exit 1; fi
	@echo "Upstream OK: $(UPSTREAM_CLI)"

codegen-app: codegen-verify-upstream
	UPSTREAM_APP_GRAPHQL="$(UPSTREAM_APP_GRAPHQL)" \
		cargo run -p graphql-codegen --example gen_app_surfaces

codegen-admin: codegen-verify-upstream
	cargo run -p graphql-codegen -- \
		"$(UPSTREAM_ADMIN_GRAPHQL)" \
		"$(OUT_GRAPHQL)/admin" \
		admin

codegen: codegen-app codegen-admin
	@echo "Codegen finished → $(OUT_GRAPHQL)"

codegen-test:
	cargo test -p graphql-codegen

codegen-check: codegen codegen-test
	cargo check -p cli-kit

codegen-verify: codegen-check
	git diff --exit-code -- $(OUT_GRAPHQL)

console-assets:
	@if [ ! -d "$(CONSOLE_SRC)" ]; then \
		echo "error: ui-extensions-dev-console not found: $(CONSOLE_SRC)"; \
		echo "hint: make console-assets UPSTREAM_CLI=/path/to/shopify/cli"; \
		exit 1; \
	fi
	cd "$(CONSOLE_SRC)" && pnpm vite build
	rm -rf "$(CONSOLE_OUT)"
	mkdir -p "$(CONSOLE_OUT)"
	cp -R "$(UPSTREAM_CLI)/packages/app/assets/dev-console/." "$(CONSOLE_OUT)/"
	@echo "Vendored dev-console assets → $(CONSOLE_OUT)"

bridge-verify-upstream:
	@if [ ! -d "$(UPSTREAM_CLI)" ]; then \
		echo "error: UPSTREAM_CLI does not exist: $(UPSTREAM_CLI)"; \
		exit 1; \
	fi
	@if [ ! -f "$(UPSTREAM_CLI)/packages/cli/bin/run.js" ]; then \
		echo "error: upstream CLI runner is missing: $(UPSTREAM_CLI)/packages/cli/bin/run.js"; \
		exit 1; \
	fi
	@if [ ! -f "$(UPSTREAM_CLI)/packages/cli/package.json" ]; then \
		echo "error: upstream CLI package metadata is missing"; \
		exit 1; \
	fi
	@echo "Bridge upstream OK: $(UPSTREAM_CLI)"

bridge-build-upstream: bridge-verify-upstream
	cd "$(UPSTREAM_CLI)" && pnpm install --frozen-lockfile
	cd "$(UPSTREAM_CLI)" && pnpm nx bundle cli

bridge-stage: bridge-build-upstream
	@if [ -z "$(NODE_RUNTIME_DIR)" ]; then \
		echo "error: NODE_RUNTIME_DIR must point to a Node.js 22.12+ runtime directory"; \
		exit 1; \
	fi
	@if [ ! -x "$(NODE_RUNTIME_DIR)/bin/node" ] && [ ! -f "$(NODE_RUNTIME_DIR)/node.exe" ]; then \
		echo "error: NODE_RUNTIME_DIR must contain bin/node (Unix) or node.exe (Windows)"; \
		exit 1; \
	fi
	rm -rf "$(BRIDGE_OUT)"
	mkdir -p "$(BRIDGE_OUT)"
	cp packaging/bridge/bridge-runner "$(BRIDGE_OUT)/bridge-runner"
	cp packaging/bridge/bridge-runner.cmd "$(BRIDGE_OUT)/bridge-runner.cmd"
	cp packaging/bridge/bridge-runner.mjs "$(BRIDGE_OUT)/bridge-runner.mjs"
	chmod +x "$(BRIDGE_OUT)/bridge-runner" "$(BRIDGE_OUT)/bridge-runner.mjs"
	cp -R "$(NODE_RUNTIME_DIR)" "$(BRIDGE_OUT)/node"
	cd "$(UPSTREAM_CLI)" && pnpm --filter @shopify/cli deploy --prod "$(abspath $(BRIDGE_NODE_OUT))"
	cd "$(UPSTREAM_CLI)" && pnpm --filter @shopify/theme deploy --prod "$(abspath $(BRIDGE_THEME_TOOLS_OUT))"
	mkdir -p "$(BRIDGE_THEME_TOOLS_OUT)/adapters"
	cp packaging/theme-tools/*.cjs "$(BRIDGE_THEME_TOOLS_OUT)/adapters/"
	@echo "Minimal bridge staged → $(BRIDGE_OUT)"

bridge-stage-full: bridge-verify-upstream
	rm -rf "$(BRIDGE_OUT)"
	mkdir -p "$(BRIDGE_OUT)"
	cp packaging/bridge/bridge-runner "$(BRIDGE_OUT)/bridge-runner"
	cp packaging/bridge/bridge-runner.cmd "$(BRIDGE_OUT)/bridge-runner.cmd"
	cp packaging/bridge/bridge-runner.mjs "$(BRIDGE_OUT)/bridge-runner.mjs"
	chmod +x "$(BRIDGE_OUT)/bridge-runner" "$(BRIDGE_OUT)/bridge-runner.mjs"
	cp -R "$(UPSTREAM_CLI)" "$(BRIDGE_NODE_OUT)"
	cd "$(UPSTREAM_CLI)" && pnpm --filter @shopify/theme deploy --prod "$(abspath $(BRIDGE_THEME_TOOLS_OUT))"
	mkdir -p "$(BRIDGE_THEME_TOOLS_OUT)/adapters"
	cp packaging/theme-tools/*.cjs "$(BRIDGE_THEME_TOOLS_OUT)/adapters/"
	rm -rf "$(BRIDGE_NODE_OUT)/.git"
	@echo "Full bridge staged → $(BRIDGE_OUT)"

bridge-archive: bridge-stage
	mkdir -p "$(DIST_DIR)"
	rm -f "$(BRIDGE_ARCHIVE)" "$(BRIDGE_ARCHIVE).sha256"
	tar -C "$(dir $(BRIDGE_OUT))" -czf "$(BRIDGE_ARCHIVE)" "$(notdir $(BRIDGE_OUT))"
	python3 -c 'import hashlib, pathlib; p = pathlib.Path("$(BRIDGE_ARCHIVE)"); print(hashlib.sha256(p.read_bytes()).hexdigest(), p.name)' > "$(BRIDGE_ARCHIVE).sha256"
	@echo "Bridge archive → $(BRIDGE_ARCHIVE)"

bridge-size:
	@du -sh "$(UPSTREAM_CLI)" 2>/dev/null || true
	@du -sh "$(BRIDGE_OUT)" 2>/dev/null || true
	@du -sh target/release/shopify target/release/create-app 2>/dev/null || true
	@du -sh "$(RELEASE_ARCHIVE)" 2>/dev/null || true

release-package: bridge-archive
	cargo build -p cli-kit --bins --release
	rm -rf "$(RELEASE_ROOT)" "$(RELEASE_ARCHIVE)"
	mkdir -p "$(RELEASE_ROOT)/bin" "$(DIST_DIR)"
	cp target/release/shopify "$(RELEASE_ROOT)/bin/shopify"
	cp target/release/create-app "$(RELEASE_ROOT)/bin/create-app"
	cp -R "$(BRIDGE_OUT)" "$(RELEASE_ROOT)/bin/bridge"
	$(MAKE) release-manifest
	tar -C "$(dir $(RELEASE_ROOT))" -czf "$(RELEASE_ARCHIVE)" "$(notdir $(RELEASE_ROOT))"
	@echo "Release archive → $(RELEASE_ARCHIVE)"

release-manifest:
	python3 scripts/release-manifest.py "$(RELEASE_ROOT)"

release-smoke:
	test -f "$(RELEASE_ROOT)/manifest.json"
	python3 -c 'import json; json.load(open("$(RELEASE_ROOT)/manifest.json", encoding="utf-8"))'
	"$(RELEASE_ROOT)/bin/shopify" version
	"$(RELEASE_ROOT)/bin/shopify" app --help
	"$(RELEASE_ROOT)/bin/shopify" theme --help
	"$(RELEASE_ROOT)/bin/shopify" hydrogen --help
	"$(RELEASE_ROOT)/bin/shopify" hydrogen dev --help
	"$(RELEASE_ROOT)/bin/shopify" plugins --help
	"$(RELEASE_ROOT)/bin/shopify" commands --all --json
	test -f "$(RELEASE_ROOT)/bin/bridge/theme-tools/adapters/theme-check.cjs"
	test -f "$(RELEASE_ROOT)/bin/bridge/theme-tools/adapters/language-server.cjs"
	test "$$(node -p "require('./$(RELEASE_ROOT)/bin/bridge/theme-tools/node_modules/@shopify/theme-check-node/package.json').version")" = "3.26.1"
	test "$$(node -p "require('./$(RELEASE_ROOT)/bin/bridge/theme-tools/node_modules/@shopify/theme-language-server-node/package.json').version")" = "2.21.3"
	"$(RELEASE_ROOT)/bin/shopify" theme check --version
	"$(RELEASE_ROOT)/bin/shopify" theme check --path packaging/fixtures/theme-check --output text
	"$(RELEASE_ROOT)/bin/shopify" theme check --path packaging/fixtures/theme-check --output json
