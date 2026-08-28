# Shopify CLI (Rust)

**Unofficial port:** This project is an independent Rust port of Shopify CLI.
It is not affiliated with, endorsed by, or supported by Shopify.

An in-progress Rust implementation of [Shopify CLI](https://github.com/Shopify/cli).
It provides the `shopify` and `create-app` binaries, with application, theme,
store, authentication, and generated GraphQL support.

## Quick start

Install Rust 1.80+ with `rustup`, then build and inspect the available commands:

```bash
cargo build -p shopirust-cli --bins
cargo run -p shopirust-cli --bin shopify -- --help
cargo run -p shopirust-cli --bin shopify -- version
```

For app-related commands, run from a directory containing `shopify.app.toml` or
pass `--path`. Theme and Admin GraphQL commands require a development store.

```bash
# Authenticate before using live Shopify APIs
cargo run -p shopirust-cli -- auth login
cargo run -p shopirust-cli -- auth status

# Explore the main command groups
cargo run -p shopirust-cli -- app --help
cargo run -p shopirust-cli -- theme --help
cargo run -p shopirust-cli -- store --help
```

## Common commands

After building, use the binary directly or replace `shopify` below with
`cargo run -p shopirust-cli --bin shopify --` while developing from source.

### Authentication and discovery

```bash
# Sign in and inspect the active session
shopify auth login
shopify auth status
shopify organization list

# Find commands and read their exact flags
shopify commands --all
shopify search deploy
shopify app deploy --help
```

### Application commands

Run these from an app directory, or pass `--path ./my-app`.

```bash
# Link local configuration to a Shopify app, then inspect it
shopify app config link --client-id <CLIENT_ID>
shopify app info
shopify app config validate

# Build, deploy, release, and inspect versions
shopify app build
shopify app deploy --allow-updates
shopify app versions list --json
shopify app release --version <VERSION_TAG> --allow-updates

# Local development and supporting workflows
shopify app dev --store my-store.myshopify.com
shopify app logs --store my-store.myshopify.com
shopify app env show
shopify app function build --path ./extensions/my-function
```

For unattended deployments, pass `--allow-updates` and, when removing remote
extensions, `--allow-deletes`.

### Theme commands

Theme commands need a development-store domain via `--store` (or the matching
environment variable).

```bash
shopify theme init my-theme
shopify theme list --store my-store.myshopify.com
shopify theme pull --store my-store.myshopify.com --path ./my-theme
shopify theme push --store my-store.myshopify.com --path ./my-theme
shopify theme dev --store my-store.myshopify.com --path ./my-theme
shopify theme check --path ./my-theme
```

### Store and Admin GraphQL commands

```bash
shopify store list --organization-id <ORG_ID>
shopify store info --store my-store.myshopify.com
shopify app execute --store my-store.myshopify.com \
  --query '{ shop { name } }'
shopify app bulk status --store my-store.myshopify.com
```

### Source-build equivalents

```bash
# Every `shopify` command can be run from the repository without installation.
cargo run -p shopirust-cli --bin shopify -- theme list --store my-store.myshopify.com
cargo run -p shopirust-cli --bin shopify -- app deploy --path ./my-app --allow-updates
cargo run -p shopirust-cli --bin shopify -- app execute --store my-store.myshopify.com \
  --query '{ shop { name } }'
```

Run `shopify <group> <command> --help` for flags and prerequisites. Some
commands remain compatibility bridges to upstream Node tooling; packaged
releases bundle the runtime needed for those commands. See [BRIDGE.md](BRIDGE.md).

## Installation

Install the public preview from crates.io. This installs the existing
`shopify` and `create-app` commands:

```bash
cargo install shopirust-cli --version 0.1.0-alpha.1 --locked
```

When installed from crates.io, native Rust commands work immediately. Before
using a compatibility command such as Hydrogen or plugin management, install
the optional verified bridge once:

```bash
shopify bridge status
shopify bridge install
```

## Testing

```bash
cargo test -p shopirust-cli --lib
cargo test -p app --lib
cargo test -p theme --lib
cargo test -p store --lib
```

The full `cli-kit` suite starts local mock servers. If a sandbox blocks local
ports, run it in an environment that permits loopback binds.

## GraphQL code generation

Generated Rust models are committed under
`crates/cli-kit/src/api/generated/graphql`. Regenerate them from a local
Shopify CLI checkout:

```bash
make codegen UPSTREAM_CLI=/path/to/shopify/cli
make codegen-check UPSTREAM_CLI=/path/to/shopify/cli
make codegen-verify UPSTREAM_CLI=/path/to/shopify/cli
```

`codegen-verify` regenerates the files and fails if the result has not been
committed. More contributor details are in [DEVELOPMENT.md](DEVELOPMENT.md).

## Workspace layout

| Crate | Role |
| --- | --- |
| `cli-kit` | CLI binaries, command wiring, API adapters |
| `cli-core` | Shared runner, flags, and errors |
| `cli-api` | Developer-platform traits and types |
| `app` | Application workflows |
| `theme` | Theme workflows |
| `store` | Store operations and authentication |
| `graphql-codegen` | GraphQL Rust-model generator |

## License

MIT — see [LICENSE](LICENSE).
