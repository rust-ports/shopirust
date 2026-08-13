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

.PHONY: help codegen-verify-upstream codegen-app codegen-admin codegen codegen-check console-assets

help:
	@echo "GraphQL codegen targets"
	@echo ""
	@echo "  make codegen              Regenerate app surfaces + admin modules"
	@echo "  make codegen-app          Regenerate app surfaces only"
	@echo "  make codegen-admin        Regenerate admin modules only"
	@echo "  make codegen-check        codegen + cargo check -p cli-kit"
	@echo "  make codegen-verify-upstream  Fail if upstream paths are missing"
	@echo "  make console-assets       Build ui-extensions-dev-console → crates/app/assets/dev-console"
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

codegen-check: codegen
	cargo check -p cli-kit

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
