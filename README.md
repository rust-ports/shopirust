# Shopify CLI (Rust)

Rust port of [Shopify CLI](https://github.com/Shopify/cli). This guide is for teammates who want to **build the binary and manually test commands that are ready today**.

Licensed under the same terms as upstream Shopify CLI — see [LICENSE](LICENSE) (MIT, Copyright 2019-present, Shopify Inc.).

## Prerequisites

- Rust **1.80+** (`rustup` toolchain)
- Network access for Auth / Admin / App Management API calls
- A Shopify Partner account (apps) and/or a development store (themes, Admin GraphQL)

Optional for app build steps:

- Node.js 22.12 or newer when running from source. Packaged releases bundle
  Node for bridge-backed commands, Theme Check, and the theme language server;
  they never resolve dependencies from the current project's `node_modules`.
- A package manager (`npm` / `yarn` / `pnpm`) if the app has a `web/` package
- `npx` available for UI extension esbuild builds

## Build and run

From the repo root:

```bash
cargo build -p cli-kit
```

Run the CLI (`shopify` binary from `cli-kit`):

```bash
cargo build -p cli-kit
cargo run -p cli-kit --bin shopify -- --help
cargo run -p cli-kit --bin shopify -- version
```

Tip: alias for shorter commands while testing:

```bash
alias shopify-rs='cargo run -p cli-kit --'
shopify-rs app --help
shopify-rs theme --help
```

## Team-release security

Store OAuth access and refresh tokens are kept in the operating system's
credential store. The metadata file under the user configuration directory is
atomically updated, locked between CLI processes, and contains no token values.
Existing plaintext sessions migrate the next time they are read; if the system
credential store is unavailable, authenticate again after enabling it.

Release artifacts are built with a checksum `manifest.json`; see
[BRIDGE.md](BRIDGE.md) for the bundled Node bridge and release staging contract.

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

# Full deploy (builds first). `--no-release` maps to Partners skipPublish (does not call AM release()).
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
# Replay does not require a linked app — logs are read from `.shopify/logs`.
# `--watch` re-runs when `dist/index.wasm` mtime changes.
cargo run -p cli-kit -- app function replay --path ./extensions/my-function --log <id>
cargo run -p cli-kit -- app function replay --path ./extensions/my-function --watch
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

# Bulk query (with --watch, results download to --output-file or stdout when COMPLETED)
cargo run -p cli-kit -- app bulk execute \
  --store my-store.myshopify.com \
  --query '{ products { edges { node { id } } } }' \
  --watch --output-file ./bulk.jsonl

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
# Streams logs and writes JSON files under `.shopify/logs` (used by function replay).
cargo run -p cli-kit -- app logs --store my-store.myshopify.com --path ./my-app

# Localhost tunnel mode (no cloudflared)
cargo run -p cli-kit -- app dev --use-localhost \
  --path ./my-app --store my-store.myshopify.com

cargo run -p cli-kit -- app dev clean --path ./my-app --store my-store.myshopify.com
```

Refresh vendored Polaris console assets from an upstream Shopify/cli checkout:

```bash
make console-assets UPSTREAM_CLI=/path/to/shopify/cli
```

`app dev` sends a live `APP_UNINSTALLED` sample (HMAC-signed) when the remote app changed, with a synthetic fallback. TTY sessions show concurrent prefixed logs, a status table, and shortcuts `p` (preview), `g` (GraphiQL), `q`/Ctrl+C (abort).

## Store + CLI meta

```bash
cargo run -p cli-kit --bin shopify -- store list --organization-id <ORG_ID>
cargo run -p cli-kit --bin shopify -- store info --store my-store.myshopify.com
cargo run -p cli-kit --bin shopify -- cache clear
cargo run -p cli-kit --bin shopify -- upgrade
cargo run -p cli-kit --bin shopify -- search deploy
cargo run -p cli-kit --bin shopify -- config autoupgrade status
cargo run -p cli-kit --bin create-app -- --name my-app --template remix
```

`cloudflared` is downloaded into `~/.shopify/` on first Auto tunnel if it is not on PATH (same pattern as mkcert). Unknown commands print a did-you-mean suggestion.

## Smoke checklist (no live Shopify)

```bash
cargo test -p app --lib
cargo test -p theme --lib
cargo test -p store --lib
cargo test -p cli-api --lib
cargo test -p cli-kit --lib
cargo test -p cli-kit --test e2e
```

## Not ready / out of scope (do not expect parity yet)

- Pixel-identical Ink DevSessionUI / Replay React components (ratatui/status-table UX instead)
- Hydrogen (`@shopify/cli-hydrogen` is an external npm plugin, not in this repo)
- Playwright journeys that hit live Shopify APIs
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
| `store` | Store list / info / execute / create / auth |
| `graphql-codegen` | Generate Rust GraphQL modules into cli-kit (committed) |

## License

MIT — [LICENSE](LICENSE), matching [Shopify/cli](https://github.com/Shopify/cli).
