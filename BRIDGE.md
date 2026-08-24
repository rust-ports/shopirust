# Shopify CLI Rust Bridge

The Rust CLI is the primary runtime. Commands that are not native Rust yet are
delegated to a bundled Node compatibility bridge.

## Runtime Contract

Bridge commands call:

```bash
bridge/bridge-runner <command-id> [...args]
```

Example:

```bash
bridge/bridge-runner hydrogen:dev --path web
```

The runner maps `hydrogen:dev` to the upstream Node CLI shape:

```bash
shopify hydrogen dev --path web
```

It preserves the current working directory, standard streams, process
environment, and exit code.

## Release Layout

The release artifact should place bridge assets beside the Rust binary:

```text
bin/shopify
bin/create-app
bin/bridge/bridge-runner
bin/bridge/bridge-runner.cmd
bin/bridge/bridge-runner.mjs
bin/bridge/node/                 # pinned Node.js 22.12+ runtime
bin/bridge/node-cli/
```

`SHOPIFY_CLI_BRIDGE_RUNNER` is a development override. Users of a packaged
release should not need to set it.

Every release bundles its own Node.js runtime. The launchers use that runtime
first; a system `node` is only a development fallback and must be 22.12 or
newer.

## crates.io installations

`cargo install` installs the Rust binaries only. On first use of a compatibility
command, the CLI tells the user to install the optional bridge explicitly:

```bash
shopify bridge status
shopify bridge install
```

The installer downloads the version- and platform-specific archive from this
repository's GitHub release, verifies its adjacent `.sha256` file, and extracts
it below the CLI cache. Set `SHOPIFY_CLI_BRIDGE_URL` (or pass `--url`) to use a
mirror. Remove the current cached bridge with `shopify bridge uninstall`.

For every `v<crate-version>` release, upload these two assets:

```text
shopify-rust-bridge-<platform>.tar.gz
shopify-rust-bridge-<platform>.tar.gz.sha256
```

Create them with `make bridge-archive`; supported platform names follow the
Node convention, such as `linux-x64`, `darwin-arm64`, and `win32-x64`.

## Staging

Stage the minimal production bridge payload from a pinned upstream checkout:

```bash
make bridge-stage UPSTREAM_CLI=/home/mohammed-niri/projects/gitCloned/cli \
  NODE_RUNTIME_DIR=/path/to/node-runtime
```

This target runs the upstream production bundle and uses `pnpm deploy --prod`
to stage only runtime package contents into `target/bridge-release/bridge`.

For debugging only, a full upstream checkout can be staged:

```bash
make bridge-stage-full UPSTREAM_CLI=/home/mohammed-niri/projects/gitCloned/cli
```

Do not use `bridge-stage-full` for release artifacts.

## Release Packaging

Create a release archive with:

```bash
make release-package UPSTREAM_CLI=/home/mohammed-niri/projects/gitCloned/cli
```

The archive is written under `target/dist/` and contains:

```text
bin/shopify
bin/create-app
bin/bridge/
```

Smoke-test the packaged layout:

```bash
make release-smoke
```

Compare payload sizes:

```bash
make bridge-size
```

The upstream CLI is private bridge implementation detail. The package must not
install the upstream Node `shopify` binary into the user's global `PATH`.

## Native Replacement Order

Hydrogen core is the first bridge surface planned for native Rust replacement:

```text
hydrogen:init
hydrogen:dev
hydrogen:build
hydrogen:deploy
hydrogen:list
hydrogen:link
hydrogen:unlink
```

After Hydrogen core, port plugin lifecycle commands, then doctor-release.
