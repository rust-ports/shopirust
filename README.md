# Shopify CLI (Rust)

Rust port of [Shopify CLI](https://github.com/Shopify/cli). This guide is for teammates who want to **build the binary and manually test commands that are ready today**.

Licensed under the same terms as upstream Shopify CLI — see [LICENSE](LICENSE) (MIT, Copyright 2019-present, Shopify Inc.).

## Prerequisites

- Rust **1.80+** (`rustup` toolchain)
- Network access for Auth / Admin / App Management API calls
- A Shopify Partner account (apps) and/or a development store (themes, Admin GraphQL)

Optional for app build steps:

- Node.js + package manager (`npm` / `yarn` / `pnpm`) if the app has a `web/` package
- `npx` available for UI extension esbuild builds

## Build and run

From the repo root:

```bash
cargo build -p cli-kit
```

Run the CLI (binary package is `cli-kit`):

```bash
cargo run -p cli-kit -- --help
cargo run -p cli-kit -- version
```

Tip: alias for shorter commands while testing:

```bash
alias shopify-rs='cargo run -p cli-kit --'
shopify-rs app --help
shopify-rs theme --help
```

Global flags:

| Flag | Description |
|------|-------------|
| `-v`, `--verbose` | More logging |
| `--no-color` | Disable color |
| `-p`, `--path <PATH>` | Project directory override |

## Auth (do this first)

Most live commands need a session.

```bash
cargo run -p cli-kit -- auth login
cargo run -p cli-kit -- auth status
cargo run -p cli-kit -- organization list
cargo run -p cli-kit -- organization list --json
cargo run -p cli-kit -- auth logout
```

## Theme commands (ready to test)

Point `--store` / `SHOPIFY_FLAG_STORE` at a development store FQDN (e.g. `my-store.myshopify.com`).

```bash
# Scaffold / inspect
cargo run -p cli-kit -- theme init my-theme
cargo run -p cli-kit -- theme list --store my-store.myshopify.com
cargo run -p cli-kit -- theme info --store my-store.myshopify.com

# Sync
cargo run -p cli-kit -- theme pull --store my-store.myshopify.com --path ./my-theme
cargo run -p cli-kit -- theme push --store my-store.myshopify.com --path ./my-theme

# Local development / preview
cargo run -p cli-kit -- theme dev --store my-store.myshopify.com --path ./my-theme
cargo run -p cli-kit -- theme preview --store my-store.myshopify.com --path ./my-theme
cargo run -p cli-kit -- theme open --store my-store.myshopify.com

# Lifecycle
cargo run -p cli-kit -- theme publish --store my-store.myshopify.com
cargo run -p cli-kit -- theme duplicate --store my-store.myshopify.com
cargo run -p cli-kit -- theme rename --store my-store.myshopify.com --new-name "Renamed"
cargo run -p cli-kit -- theme delete --store my-store.myshopify.com --force
cargo run -p cli-kit -- theme share --store my-store.myshopify.com
cargo run -p cli-kit -- theme package --path ./my-theme

# Extra
cargo run -p cli-kit -- theme check --path ./my-theme
cargo run -p cli-kit -- theme console --store my-store.myshopify.com
cargo run -p cli-kit -- theme profile --store my-store.myshopify.com --path ./my-theme
cargo run -p cli-kit -- theme language-server
```

Use `theme <command> --help` for the full flag set (`--theme` / `-t`, `--development`, `--json`, `--only`, `--ignore`, etc.).

## App commands (ready to test)

Work from an app directory that has `shopify.app.toml` (or pass `--path` / `-c`).

Preferred platform path for deploy/release/versions: **App Management / Developer Dashboard** (`client_id` linked config). Partners remains deploy-only for some flows.

### Config + info

```bash
cargo run -p cli-kit -- app info --path ./my-app
cargo run -p cli-kit -- app info --path ./my-app --json

cargo run -p cli-kit -- app config link --path ./my-app --client-id <CLIENT_ID>
cargo run -p cli-kit -- app config use --path ./my-app
cargo run -p cli-kit -- app config pull --path ./my-app
cargo run -p cli-kit -- app config validate --path ./my-app
```

### Build / deploy / release / versions

```bash
cargo run -p cli-kit -- app build --path ./my-app
cargo run -p cli-kit -- app build --path ./my-app --skip-dependencies-installation

# Deploy without rebuilding extensions already on disk
cargo run -p cli-kit -- app deploy --path ./my-app --no-build --allow-updates --allow-deletes

# Full deploy (builds first), create version but do not release
cargo run -p cli-kit -- app deploy --path ./my-app --no-release --message "test deploy" --allow-updates

cargo run -p cli-kit -- app versions list --path ./my-app
cargo run -p cli-kit -- app versions list --path ./my-app --json

cargo run -p cli-kit -- app release --path ./my-app --version <VERSION_TAG> --allow-updates
```

Non-interactive / CI: pass `--allow-updates` (and `--allow-deletes` when the diff removes extensions). Interactive TTY can confirm prompts.

### Functions

Run from the app root or a function extension directory (`--path`). Toolchain binaries (function-runner, javy, trampoline, wasm-opt) download into the CLI cache on first use.

```bash
cargo run -p cli-kit -- app function build --path ./extensions/my-function
cargo run -p cli-kit -- app function info --path ./extensions/my-function --json
cargo run -p cli-kit -- app function typegen --path ./extensions/my-function
cargo run -p cli-kit -- app function schema --path ./extensions/my-function
cargo run -p cli-kit -- app function run --path ./extensions/my-function --input ./input.json
cargo run -p cli-kit -- app function replay --path ./extensions/my-function --log <id> --watch=false
```

### Admin GraphQL: execute + bulk

Requires store auth (`--store` / `SHOPIFY_FLAG_STORE`). Bulk API version defaults to `2026-01` (minimum).

```bash
cargo run -p cli-kit -- app execute \
  --store my-store.myshopify.com \
  --query '{ shop { name } }'

cargo run -p cli-kit -- app execute \
  --store my-store.myshopify.com \
  --query-file ./query.graphql \
  --output-file ./result.json

# Bulk query
cargo run -p cli-kit -- app bulk execute \
  --store my-store.myshopify.com \
  --query '{ products { edges { node { id } } } }' \
  --watch

# Bulk mutation (needs --variables or --variable-file JSONL)
cargo run -p cli-kit -- app bulk execute \
  --store my-store.myshopify.com \
  --query 'mutation ($input: ProductInput!) { productCreate(input: $input) { product { id } } }' \
  --variable-file ./vars.jsonl

cargo run -p cli-kit -- app bulk status --store my-store.myshopify.com
cargo run -p cli-kit -- app bulk status --store my-store.myshopify.com --id <ID> --json
cargo run -p cli-kit -- app bulk cancel --store my-store.myshopify.com --id <ID>
```

Numeric bulk IDs are normalized to `gid://shopify/BulkOperation/<id>`.

## Suggested smoke checklist

1. `auth login` → `organization list` succeeds.
2. Theme: `theme list` / `theme pull` / `theme push` against a dev store.
3. App: `app config link` → `app info` → `app versions list`.
4. App: `app deploy --no-build --allow-updates` (App Management) creates a version.
5. App: `app execute --query '{ shop { name } }'` against a store.
6. App: `app function build` / `app function info --json` for a function extension.
7. App: `app env show` / `app webhook trigger --help` / `app logs sources`.
8. App: `app dev --use-localhost --store <store>` (requires linked app; `cloudflared` for auto tunnel).
9. `cargo test -p app --lib` and `cargo test -p theme --lib` (unit / wiremock).

See [APP_PARITY_GAPS.md](APP_PARITY_GAPS.md) for remaining depth vs upstream Vitest.

### App liveloop / env / logs (ready to test)

```bash
cargo run -p cli-kit -- app env show --path ./my-app
cargo run -p cli-kit -- app env pull --path ./my-app

# Flags are optional; omitted topic / api-version / address / delivery-method are prompted.
# Localhost POSTs from the CLI; https / pubsub:// / EventBridge ARNs enqueue via WebhooksClient.
cargo run -p cli-kit -- app webhook trigger \
  --topic products/create \
  --api-version 2025-01 \
  --address http://localhost:3000/webhooks

cargo run -p cli-kit -- app webhook trigger \
  --topic orders/create \
  --api-version 2025-01 \
  --delivery-method event-bridge \
  --address arn:aws:events:us-east-1::event-source/aws.partner/shopify.com/1/source

cargo run -p cli-kit -- app logs sources --path ./my-app
cargo run -p cli-kit -- app logs --store my-store.myshopify.com --path ./my-app

# Localhost tunnel mode (no cloudflared)
cargo run -p cli-kit -- app dev --use-localhost \
  --path ./my-app --store my-store.myshopify.com

cargo run -p cli-kit -- app dev clean --path ./my-app --store my-store.myshopify.com
```

## Not ready / out of scope (do not expect parity yet)

- Full Polaris `ui-extensions-dev-console` static asset bundle (dev console is a functional HTML/WS stub)
- Store / Hydrogen surfaces
- CLI meta (`upgrade`, `cache`, notifications, did-you-mean)
- Full Partners parity for release / signed upload / versions (App Management is the primary path)
- Playwright-class E2E suite
## Regenerate GraphQL

Rust GraphQL modules under `crates/cli-kit/src/api/generated/graphql/` are produced from an upstream [Shopify/cli](https://github.com/Shopify/cli) checkout (`.graphql` + `generated/*.ts` + `types.d.ts`). Refresh them with the root Makefile:

```bash
make help

# Point at your local Shopify/cli clone
make codegen UPSTREAM_CLI=/path/to/shopify/cli

# Same, then verify cli-kit still compiles
make codegen-check UPSTREAM_CLI=/path/to/shopify/cli

# App surfaces or admin only
make codegen-app UPSTREAM_CLI=/path/to/shopify/cli
make codegen-admin UPSTREAM_CLI=/path/to/shopify/cli
```

Defaults assume a sibling checkout at `../gitCloned/cli`. Override with `UPSTREAM_CLI`, or set `UPSTREAM_APP_GRAPHQL` / `UPSTREAM_ADMIN_GRAPHQL` directly.

If upstream TypeScript artifacts are stale, run Shopify/cli’s own GraphQL codegen first, then re-run `make codegen`.

## Workspace layout

| Crate | Role |
|-------|------|
| `cli-kit` | Binary + command wiring + API adapters |
| `cli-core` | Shared CLI runner / flags |
| `cli-api` | Developer-platform traits / types |
| `app` | App domain (load, config, build, deploy, bulk) |
| `theme` | Theme domain |
| `graphql-codegen` | Generate Rust GraphQL modules |

## License

MIT — [LICENSE](LICENSE), matching [Shopify/cli](https://github.com/Shopify/cli).
