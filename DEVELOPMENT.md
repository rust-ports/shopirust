# Developing Shopify CLI (Rust)

**Unofficial port:** This project is an independent Rust port of Shopify CLI.
It is not affiliated with, endorsed by, or supported by Shopify.

## Prerequisites

- Rust 1.80+ via `rustup`
- Node.js 22.12+ and `pnpm` only when rebuilding bridge or upstream assets
- A local checkout of [Shopify/cli](https://github.com/Shopify/cli) for
  GraphQL generation and bridge packaging

The default upstream checkout location is `../gitCloned/cli`; override it with
`UPSTREAM_CLI=/path/to/shopify/cli` for every Make target.

## Daily workflow

```bash
# Format and compile all cli-kit targets
cargo fmt --check
cargo check -p cli-kit --all-targets --all-features

# Run focused crates while iterating
cargo test -p cli-kit --lib
cargo test -p app --lib
cargo test -p theme --lib
cargo test -p store --lib
```

The CLI suite uses Wiremock and needs permission to bind local loopback ports.
Avoid live Shopify calls in normal unit tests.

## GraphQL generated code

Do not hand-edit files under `crates/cli-kit/src/api/generated/graphql/`.
Instead, refresh their upstream source artifacts and run:

```bash
make codegen UPSTREAM_CLI=/path/to/shopify/cli
make codegen-check UPSTREAM_CLI=/path/to/shopify/cli
make codegen-verify UPSTREAM_CLI=/path/to/shopify/cli
```

`codegen-check` regenerates then compiles `cli-kit`. `codegen-verify` also
fails when regeneration changes committed output, making it suitable for CI.

Generated wire models should stay at the API boundary. When the public domain
model intentionally differs, map the generated response into a handwritten
domain type and add a compatibility test for any known legacy API response.

## Packaging and bridge tooling

```bash
make console-assets UPSTREAM_CLI=/path/to/shopify/cli
make bridge-stage UPSTREAM_CLI=/path/to/shopify/cli NODE_RUNTIME_DIR=/path/to/node-runtime
make bridge-archive UPSTREAM_CLI=/path/to/shopify/cli NODE_RUNTIME_DIR=/path/to/node-runtime
make release-package UPSTREAM_CLI=/path/to/shopify/cli NODE_RUNTIME_DIR=/path/to/node-runtime
make release-smoke
```

`bridge-stage` requires a distributable Node runtime directory containing
`bin/node` on Unix or `node.exe` on Windows. Release packages include a
checksum manifest; see [BRIDGE.md](BRIDGE.md) for its layout.

Upload both `target/dist/shopify-rust-bridge-<platform>.tar.gz` and its
`.sha256` sidecar to the GitHub release tagged `v<crate-version>`. The
crates.io-installed CLI downloads those assets only after an explicit
`shopify bridge install` request.

## Commit expectations

- Keep generated updates with their source operation or schema change.
- Add tests alongside behavior changes.
- Run formatting, the affected crate tests, and `git diff --check` before
  committing.
