# DEVOPS.md: Rust Rewrite DevOps Plan

> **Source documents:** CLI-MAP.md (architecture), API-shopify.md (API surfaces), PORT.md (Rust plan), TEST-MAP.md (test coverage)
>
> **CI platform:** GitHub Actions — CLI-MAP.md is silent on CI platform, but PORT.md §6.5 explicitly references "GitHub Actions `actions/cache` for `~/.cargo` and `target/` directories." No other platform was found in any of the four documents. GitHub Actions is confirmed.
>
> Every section is derived exclusively from these four documents. Anything not traceable to them is in §6 (Open Questions).

---

## 1. Build Pipeline

### Nx Target Mapping

Maps every Nx target from CLI-MAP.md §2 (Nx task pipeline) to its Rust/CI equivalent.

| Nx Target (CLI-MAP §2) | JS Command | Rust Equivalent | Runs In | Produces |
|---|---|---|---|---|
| `clean` | `nx clean` | `cargo clean` | Development, CI | Clean workspace state |
| `build` | `tsc -b ./tsconfig.build.json` | `cargo check --workspace` (fast CI) / `cargo build --workspace` (dev) | Development, CI | Compilation gate; `cargo build` produces `target/debug/*.rlib` per crate |
| `type-check` | `tsc` (type checking) | Inherent in `cargo check` — Rustc type-checks all code. No separate step needed. | Development, CI | Gate (compilation succeeds) |
| `lint` | ESLint (per-package) | `cargo clippy --workspace -- -D warnings` | Development, CI | Lint gate |
| `test` | `vitest run` | `cargo nextest run` (or `cargo test` if nextest unavailable) | Development, CI | Test pass/fail gate |
| `graphql-codegen` | `@graphql-codegen/*` generates TypeScript from `.graphql` files | `build.rs` via `cynic-codegen` — runs automatically on every `cargo build` (PORT §6.2) | Development, CI | Generated Rust `QueryFragment` structs |
| `refresh-manifests` | `oclif manifest` + `oclif readme` — generates command manifest + README | `clap_complete` shell completion scripts (generated offline, committed to repo) | CI (release) | Shell completion files for bash/zsh/fish/powershell |
| `bundle` | esbuild bundle → single `.js` file per package | `cargo build --release --package cli` → single `shopify` binary. Also `cargo build --release --package create-app` → `shopify-create` binary (PORT §6.3) | CI (release) | Stripped binary in `target/release/shopify` |
| `^build` (dep build) | Nx resolves topological dependencies | `cargo` does this inherently — `--workspace` builds all crates in dependency order | Development, CI | (implicit in build) |

### Resolved: `graphql-codegen` → `build.rs` + `cynic-codegen`

JS approach (CLI-MAP §2): `.graphql` files → `@graphql-codegen/*` → generated TypeScript documents in `generated/` directories. A separate CLI invocation (`graphql-codegen`) writes to disk.

Rust approach (PORT §6.2): Same `.graphql` files (per API surface) feed into `cynic-codegen` via each crate's `build.rs`. The codegen happens automatically every time `cargo build` runs:

```rust
// In crates/cli-kit/build.rs (PORT §6.2)
fn main() {
    cynic_codegen::register_schema("partners")
        .from_schema_file("api/partners/schema.graphql")
        .unwrap();
    cynic_codegen::register_schema("admin")
        .from_schema_file("api/admin/schema.graphql")
        .unwrap();
    cynic_codegen::register_schema("app_management")
        .from_schema_file("api/app_management/schema.graphql")
        .unwrap();
    // ... etc for each API surface
}
```

**Implication for CI:** `cargo build` (step 2 in CI) implicitly runs codegen. No separate CI step needed — the codegen is part of the compilation gate. If a `.graphql` query references a field that doesn't exist in `schema.graphql`, the build fails with a cynic compiler error.

**Implication for local dev:** Codegen runs on every `cargo build`, adding overhead to the edit-compile loop. See §5 (Local Development) for mitigation.

### Resolved: `refresh-manifests` → Shell Completions

JS behavior (CLI-MAP §2): `refresh-manifests` runs `oclif manifest` (generates `oclif.manifest.json` — a JSON index of all commands, their flags, topics, and hooks) and `oclif readme` (generates command reference in README). The manifest is used for lazy command loading at runtime (CLI-MAP §4.1: `custom-oclif-loader.ts` loads commands from manifest lazily).

Rust replacement: `clap` has no runtime manifest — all commands are statically registered at compile time. There is no lazy loading equivalent. The `refresh-manifests` analogue is generating shell completion files via `clap_complete`:

```rust
// In cli crate, a hidden subcommand or build script:
use clap_complete::{generate_to, Shell};
generate_to(Shell::Bash, &mut cmd, "shopify", "completions/")?;
generate_to(Shell::Zsh, &mut cmd, "shopify", "completions/")?;
generate_to(Shell::Fish, &mut cmd, "shopify", "completions/")?;
```

These are generated in CI before a release, committed to the repo, and installed alongside the binary. There is no README auto-generation — the Rust documentation (`cargo doc`) serves as command reference.

| Aspect | JS (oclif) | Rust (clap_complete) |
|---|---|---|
| Purpose | Lazy command loading + docs | Shell completion + docs |
| When generated | Every build (part of refresh-manifests) | Before release only |
| Runtime consumption | oclif reads manifest JSON to resolve commands | No runtime consumption — statically linked |
| README | Auto-generated from manifest | `cargo doc` or manual |
| CI step | `nx run refresh-manifests` | Release workflow calls `clap_complete` generator |

### Resolved: `bundle` → Post-Processing

JS `bundle` (CLI-MAP §2): esbuild bundles per-package into a single `.js` file. The result is a Node.js script with all dependencies inlined.

Rust `cargo build --release` (PORT §6.3): Produces a statically linked native binary at `target/release/shopify`. Because Rust uses static linking by default (all workspace crates compiled and linked together), there is no bundling step. The produced binary is self-contained.

**Post-processing decisions:**

| Action | Apply? | Rationale |
|---|---|---|
| `strip` | ✅ Yes — `strip target/release/shopify` | Default `cargo build --release` includes debug symbols in the binary by default (DWARF sections). Stripping removes these, reducing binary size by ~30-50%. Done in CI after build. |
| `upx` | ❌ No | UPX compression adds startup decompression overhead. The CLI is latency-sensitive (auth flows, file operations). JS bundles were already up to ~30MB uncompressed; Rust binary at comparable or smaller size without compression is acceptable. |
| Code signing | ✅ macOS: `codesign` | Required for macOS distribution. Done in release CI. |
| Notarization | ✅ macOS: `xcrun notarytool` | Required for macOS distribution. Done in release CI. |

**Binary size estimate:** Based on PORT §6.1 dependencies (tokio, reqwest, clap, ratatui, cynic, governor, etc.), the stripped binary at `target/release/shopify` is expected at **15–25 MB** (typical for a CLI with tokio + reqwest + ratatui). The `create-app` binary is a separate, smaller binary at ~8–12 MB.

---

## 2. CI Pipeline

### Pipeline Overview

Eight ordered gates, executed on every push/PR (unless noted otherwise). Cache keys are documented per step. Parallelization is noted where steps can run concurrently.

```
┌─ Gate 1: Format    ─┐  (parallelizable with 2, 3)
├─ Gate 2: Compile    ─┤  (parallelizable with 1, 3; depends on: none)
├─ Gate 3: Lint       ─┤  (parallelizable with 1, 2)
├─ Gate 4: Unit+Int   ─┤  (depends on: 2)
├─ Gate 5: Schema     ─┤  (fast check, depends on: 2)
├─ Gate 6: Release    ─┤  (depends on: 2, 4, 5 pass)
├─ Gate 7: E2E        ─┤  (depends on: 6)
└─ Gate 8: Pre-rel    ─┘  (manual dispatch or tag push; depends on: all above)
```

### Step-by-Step Specification

#### Gate 1: Format Check

| Property | Value |
|---|---|
| **Step name** | `cargo fmt --check` |
| **Exact command** | `cargo fmt --all -- --check` |
| **What it gates** | Code style consistency. Blocks CI if any file is not formatted per `rustfmt` defaults. |
| **Parallel with** | Gates 2, 3 (no dependencies) |
| **Cache key** | None (no artifacts, just source scan) |
| **Cached** | Nothing |

#### Gate 2: Compile Check

| Property | Value |
|---|---|
| **Step name** | `cargo check --workspace` |
| **Exact command** | `cargo check --workspace --all-targets` |
| **What it gates** | Type correctness. Blocks CI if any crate fails to compile (including GraphQL codegen via `build.rs`). Using `cargo check` instead of `cargo build` for speed — it skips codegen for the final binary artifact. |
| **Parallel with** | Gates 1, 3 |
| **Cache key** | `v1-cargo-check-${{ hashFiles('**/Cargo.lock') }}-${{ runner.os }}-${{ env.RUSTC_VERSION }}-${{ env.TARGET_TRIPLE }}` |
| **Cached** | `~/.cargo/registry/`, `~/.cargo/git/`, `target/` (incremental compilation artifacts). **Must invalidate** when `Cargo.lock`, `rust-toolchain.toml`, or any `.rs` source changes. |

#### Gate 3: Lint

| Property | Value |
|---|---|
| **Step name** | `cargo clippy -- -D warnings` |
| **Exact command** | `cargo clippy --workspace --all-targets -- -D warnings` |
| **What it gates** | Code quality. `-D warnings` turns every clippy lint into a hard error. Blocks CI on unused variables, needless clones, enum variant size differences, etc. |
| **Parallel with** | Gates 1, 2 |
| **Cache key** | Same as Gate 2 (relies on `target/` from `cargo check`) |
| **Cached** | Same as Gate 2. Shares cache with Gate 2 — `cargo check` and `cargo clippy` share incremental compilation artifacts. |

#### Gate 4: Unit + Integration Tests

| Property | Value |
|---|---|
| **Step name** | `cargo nextest run` |
| **Exact command** | `cargo nextest run --workspace --profile ci` |
| **What it gates** | All unit tests (in `src/`) and integration tests (in `tests/` per crate). Gates correctness of logic, wiremock mock servers, and async behavior. |
| **Depends on** | Gate 2 (must compile first) |
| **Parallel with** | Gate 5 (can run concurrently if both depend on Gate 2) |
| **Cache key** | `v1-cargo-nextest-${{ hashFiles('**/Cargo.lock') }}-${{ runner.os }}-${{ env.RUSTC_VERSION }}` |
| **Cached** | `~/.cargo/registry/`, `target/`, `~/.cargo/bin/nextest` (install cache). Test results (nextest archive) if upload configured. |

#### Gate 5: GraphQL Schema Drift Check

| Property | Value |
|---|---|
| **Step name** | `graphql schema drift check` |
| **Exact command** | `cargo check --workspace` (re-runs cynic build.rs against checked-in schema snapshots). If schema files were updated, a diff check is required: `git diff --exit-code crates/cli-kit/api/*/schema.graphql` |
| **What it gates** | Catches when a remote Shopify API schema changes a field that the CLI queries. The cynic build.rs generates `QueryFragment` structs; if a queried field is removed or renamed, compilation fails. Snapshot files (`schema.graphql`) are committed — CI checks they haven't drifted from the committed version. |
| **Depends on** | Gate 2 (compilation succeeds, which means all queries compile against committed schemas) |
| **Two-phase check:** | 1. `cargo check` (already happened in Gate 2, so this step is normally a no-op). 2. `git diff --exit-code crates/cli-kit/api/*/schema.graphql` — ensures schema files are not modified without review. |
| **Parallel with** | Gate 4 (both depend on Gate 2, no cross-dependency) |
| **Cache key** | None (already covered by Gates 2 and 4 caches) |
| **Cached** | Nothing |

The drift detection flow (derived from PORT §6.2 cynic approach):

```
Remote API → fetch latest schema.graphql → update file in repo → PR review
                                                              ↓
On `cargo build`: cynic-codegen compares each .graphql query   ↓
against schema.graphql. If field missing → compilation error.  ↓
                                              ↓
CI: git diff --exit-code on schema.graphql files ensures       ↓
schema updates are intentional (committed, not auto-generated).↓
                                              ↓
Developer: "I see `businessName` was removed from Partners     ↓
API. I need to update our find_org.graphql to match."          ↓
```

**CI failure example:**
```
error[E0412]: cannot find type `BusinessName` in this scope
  --> crates/cli-kit/src/api/partners/queries/find_org.rs:10:22
   |
10 |     pub business_name: String,
   |                    ^^^^ help: a type with a similar name exists: `OrganizationName`
```

#### Gate 6: Release Build

| Property | Value |
|---|---|
| **Step name** | `cargo build --release` |
| **Exact command** | `cargo build --release --package cli && cargo build --release --package create-app` |
| **What it gates** | Produces the final `shopify` and `shopify-create` binaries. Run on every CI to catch build regressions in release mode (different optimization paths may surface `unsafe` UB or codegen differences from debug mode). |
| **Depends on** | Gates 2, 4, 5 all passing |
| **Cache key** | `v1-cargo-release-${{ hashFiles('**/Cargo.lock') }}-${{ runner.os }}-${{ env.RUSTC_VERSION }}-${{ env.TARGET_TRIPLE }}` |
| **Cached** | `~/.cargo/registry/`, `target/` (separate from debug `target/` to avoid LTO recompilation). |
| **Post-processing** | `strip target/release/shopify` and `strip target/release/shopify-create`. On macOS: `codesign` + notarization. |

#### Gate 7: E2E Tests

| Property | Value |
|---|---|
| **Step name** | `e2e tests (wiremock)` |
| **Exact command** | `cargo nextest run --test e2e --profile ci` (E2E tests are in `crates/cli/tests/e2e/`) |
| **What it gates** | CLI binary end-to-end behavior: app lifecycle (init → deploy → release), auth login simulation, command flag parsing, output format. |
| **Depends on** | Gate 6 (needs release binary) |
| **Setup** | 1. `cargo build --release --package cli` (already from Gate 6). 2. `wiremock` mock servers are started by each test (embedded, no external service). 3. Environment: `SHOPIFY_CLI_PARTNERS_TOKEN=test-token` to bypass real auth. |
| **Cache key** | `v1-cargo-release-${{ ... }}` (same as Gate 6) |
| **Cached** | Same as Gate 6. E2E tests themselves are NOT cached — they execute fresh each run. |

**Live API E2E tests** (TEST-MAP §5: "requires valid API credentials/tokens") are run separately:

| Property | Value |
|---|---|
| **Step name** | `e2e tests (live)` |
| **Exact command** | `cargo nextest run --test e2e-live --profile ci` |
| **When** | Nightly or on-demand (not on every PR). Requires encrypted API credentials in GitHub Actions secrets. |
| **Cache** | **Must NOT be cached** — live API tests verify real API behavior. Results are intended to vary with remote state. |

#### Gate 8: Pre-Release Validation Gate

| Property | Value |
|---|---|
| **Step name** | `doctor-release theme -e <environment>` |
| **Exact command** | `shopify doctor-release theme -e <environment> [-s <store>] [--password <token>]` — source: `packages/cli/src/cli/commands/doctor-release/theme/index.ts` |
| **What it gates** | Theme command workflow integrity. Validates that `shopify theme init` and `shopify theme push --unpublished` produce expected output. NOT an API connectivity check — the speculative list in earlier revisions was incorrect. |
| **When** | On tag push (`v*`) or manual workflow dispatch. Requires `-e`/`--environment` flag and a live store connection for the push suite (store-connected tests flagged via `static requiresStore = true`). |
| **Depends on** | Gates 2–7 all passing |
| **Cache** | **Must NOT be cached** — gates on current live store state. |

The `doctor-release` command (hidden, `packages/cli/src/cli/commands/doctor-release/doctor-release.ts`) is a thin dispatcher. Only the `theme` subcommand exists. The Rust equivalent would be `cargo run --package cli -- doctor-release theme -e production`.

The actual checks are two test suites in `packages/cli/src/cli/services/doctor-release/theme/runner.ts`, run in order — stops on first test failure:

**Suite 1: `ThemeInitTests`** (`packages/cli/src/cli/services/doctor-release/theme/tests/init.ts`)

| # | Check Name | What It Validates | Pass | Fail |
|---|---|---|---|---|
| 1.1 | `init creates theme directory` | Runs `shopify theme init <name>` interactively, asserts exit code 0 | Binary created, exit 0 | Creation failed, non-zero exit |
| 1.2 | `essential theme files exist` | Asserts `layout/theme.liquid`, `config/settings_schema.json`, `templates/index.json` exist on disk | All 3 files present | Any file missing |
| 1.3 | `theme directories exist` | Asserts `sections`, `snippets`, `assets`, `locales` directories exist | All 4 dirs present | Any directory missing |
| 1.4 | `layout/theme.liquid has valid content` | Asserts file content matches `/<!doctype html>\|<html\|{{ content_for_header }}/i` | Regex matches file content | No match found |

**Suite 2: `ThemePushTests`** (`packages/cli/src/cli/services/doctor-release/theme/tests/push.ts`)

| # | Check Name | What It Validates | Pass | Fail |
|---|---|---|---|---|
| 2.1 | `push creates unpublished theme` | Runs `shopify theme push --unpublished --json -t <name>` with store creds, asserts exit 0 | Exit 0, valid JSON | Non-zero exit or invalid JSON |
| 2.1a | *(sub-assertion)* `theme.id` is a number | JSON output has `theme.id` of type `number` | `typeof id === 'number'` | Missing or non-numeric |
| 2.1b | *(sub-assertion)* Theme role is `unpublished` | `theme.role === 'unpublished'` | Role matches | Role is `published`, `development`, etc. |
| 2.1c | *(sub-assertion)* Editor URL is provided | `theme.editor_url` includes `/admin/themes/` | URL contains path | URL missing or malformed |
| 2.1d | *(sub-assertion)* Preview URL is provided | `theme.preview_url` includes `preview_theme_id=` | URL contains query param | URL missing or malformed |

**Framework** (`packages/cli-kit/src/public/node/doctor/framework.ts`):
- Each check runs CLI commands via `this.run()` (captured output) or `this.runInteractive()` (stdin: inherit).
- Assertions record `{description, passed, expected, actual}` — results are structured `TestResult` objects with `status: 'passed' | 'failed'`.
- The runner stops at the first failing test (`result.status === 'failed'` → short-circuit return).
- The theme command (`index.ts`) sets `process.exitCode = 1` if any result has `status === 'failed'`.

**Rust equivalent:** A `#[cfg(feature = "doctor")]` integration test in `crates/cli/tests/` that runs the actual `shopify` binary via `assert_cmd`/`Command::cargo_bin()`, performing the same file-existence and JSON-output assertions. The `requiresStore` flag gates the push suite behind a `SHOPIFY_CLI_STORE` env var. The Rust version would use `assert_fs` for temp directories instead of the working directory.

---

## 3. Release Pipeline

### Target Artifacts

| Binary | Source Crate (PORT §1) | Default Target | Additional Targets |
|---|---|---|---|
| `shopify` | `crates/cli` (bin) | `x86_64-unknown-linux-gnu` (Linux), `x86_64-apple-darwin` (macOS Intel), `aarch64-apple-darwin` (macOS ARM) | `x86_64-unknown-linux-musl` (static musl), `aarch64-unknown-linux-gnu` (ARM Linux) |
| `shopify-create` | `crates/create-app` (bin) | Same triples as above | Same as above |

**Why these targets:** PORT §6.1 specifies `reqwest` with `rustls-tls` feature (no OpenSSL dependency), enabling clean cross-compilation to `*-linux-musl` targets. The primary distribution targets match the platforms Shopify merchants and developers use: Linux (CI/CD servers, Codespaces), macOS (local development), and ARM Macs (Apple Silicon).

### Distribution Channel

**Primary: GitHub Releases + shell install script**

```
GitHub Release v3.0.0
├── shopify-x86_64-unknown-linux-gnu.tar.gz
├── shopify-x86_64-apple-darwin.tar.gz
├── shopify-aarch64-apple-darwin.tar.gz
├── shopify-create-x86_64-unknown-linux-gnu.tar.gz
├── shopify-create-x86_64-apple-darwin.tar.gz
├── shopify-create-aarch64-apple-darwin.tar.gz
├── shopify-x86_64-unknown-linux-musl.tar.gz
├── checksums.txt (SHA-256)
└── install.sh
```

**Install script (`install.sh`):**
```bash
curl -fsSL https://github.com/Shopify/cli/releases/latest/download/install.sh | bash
```

Detects OS/arch, downloads appropriate tarball, extracts to `~/.shopify-cli/bin/` or `/usr/local/bin/`.

**Secondary: Homebrew formula (macOS)**

```ruby
# Formula/shopify-cli.rb
class ShopifyCli < Formula
  desc "Shopify CLI"
  homepage "https://shopify.dev"
  version "3.0.0"

  if Hardware::CPU.arm?
    url "https://github.com/Shopify/cli/releases/download/v3.0.0/shopify-aarch64-apple-darwin.tar.gz"
  else
    url "https://github.com/Shopify/cli/releases/download/v3.0.0/shopify-x86_64-apple-darwin.tar.gz"
  end

  def install
    bin.install "shopify"
  end
end
```

**Not: crates.io.** The `cli` and `create-app` binaries are end-user tools, not libraries. Publishing them on crates.io (`cargo install shopify-cli`) would require users to have Rust toolchain installed and compile from source (~2–5 minutes compile time). GitHub Releases provides a zero-dependency download.

**Comparison to JS distribution:**
- JS: `npm install -g @shopify/cli` → downloads from npm registry → Node.js interpreter required → ~300KB download + ~30MB runtime
- Rust: `curl ... | sh` → downloads from GitHub Releases → no runtime dependencies → ~20MB download, zero install-time dependencies

### Version Strategy

**Version source:** `Cargo.toml` (`crates/cli/Cargo.toml`), read at compile time via `env!("CARGO_PKG_VERSION")`. Equivalent to JS `CLI_KIT_VERSION` (CLI-MAP §5.2).

**Version scheme:** Semantic versioning (`major.minor.patch`). CLI-MAP §4.8 `version` command "Show CLI version" — the Rust binary prints `shopify version` from `CARGO_PKG_VERSION`. The `upgrade` command "Upgrade CLI" — checks current version against the latest GitHub Release tag.

**What drives version bumps:**

| Bump | Trigger | Example |
|---|---|---|
| Patch | Bug fixes, dependency updates, documentation changes | `3.0.0` → `3.0.1` |
| Minor | New commands, new API surface support, new extension types | `3.0.1` → `3.1.0` |
| Major | Breaking CLI flag changes, removed commands, breaking API surface changes | `3.1.0` → `4.0.0` |

**What triggers a release:**

| Trigger | Action | Version Bump |
|---|---|---|
| Tag push (`v*`) | Full release workflow: build all targets, sign, create GitHub Release | Manual (tag author chooses) |
| Manual workflow dispatch | Same as tag push but on-demand from GitHub UI | Manual (workflow input) |
| Nightly (optional) | Build + test but no release | N/A (pre-release artifacts only) |

**PR merge → release flow:**
```
1. PR merged to main
2. Maintainer: `git tag v3.1.0 && git push origin v3.1.0`
3. GitHub Actions: Gate 1-8 run automatically
4. On success: GitHub Release created with tarballs and install.sh
5. Homebrew formula PR opened automatically (or manually)
```

---

## 4. Caching Strategy

### sccache Configuration

`sccache` is a compiler cache that caches compilation artifacts across builds. Configured in CI via environment variables:

```yaml
- name: Setup sccache
  uses: mozilla-actions/sccache-action@v0
  with:
    version: "latest"

- name: Configure sccache
  run: |
    echo "SCCACHE_GHA_ENABLED=true" >> $GITHUB_ENV
    echo "RUSTC_WRAPPER=sccache" >> $GITHUB_ENV
```

### Cache Key Per Step

All cache keys follow this base structure:

```
v1-{step}-{lockfile_hash}-{os}-{rustc}-{target}
```

| Step | Cache Key | What is Cached | Notes |
|---|---|---|---|
| Gate 2 (check) | `v1-check-{lockfile}-{os}-{rustc}-{target}` | `~/.cargo/registry/`, `~/.cargo/git/`, `target/` | `Cargo.lock` hash ensures dep changes invalidate. `rustc` version ensures compiler version changes invalidate. |
| Gate 3 (clippy) | Same as check | Shares `target/` with check | Clippy reuses check's incremental artifacts — clippy lints don't invalidate the cache. |
| Gate 4 (test) | `v1-test-{lockfile}-{os}-{rustc}` | `~/.cargo/registry/`, `target/`, `~/.cargo/bin/nextest` | Test binaries are separate from check artifacts. `--target` not needed if same as runner. |
| Gate 6 (release) | `v1-release-{lockfile}-{os}-{rustc}-{target}` | `~/.cargo/registry/`, `target/` (release profile) | Separate cache from debug `target/` to avoid LTO recompilation when switching between debug and release. |
| Gate 7 (E2E wiremock) | Same as release | Same as release | E2E uses the release binary; no separate cache needed. |

### What Must NOT Be Cached

| Item | Reason (derived from TEST-MAP.md §5) |
|---|---|
| **Live API E2E test results** | TEST-MAP §5: E2E "requires valid API credentials/tokens" and hits real Shopify environments. Results vary with remote state. Caching would mask API regressions. |
| **`doctor-release` output** | Gate 8 validates current remote API state. Caching would defeat the purpose. |
| **Schema drift check snapshots** | Schema files are committed, not generated. CI checks `git diff --exit-code` — caching would skip the diff. |

### Cache Hierarchy

```
GitHub Actions cache
  size limit: ~10 GB per repo
  │
  ├── ~/.cargo/registry/     (shared across all steps)
  │     Cache key: lockfile hash only (OS-independent)
  │     Restore: always
  │
  ├── target/debug/          (Gates 2, 3, 4)
  │     Cache key: lockfile hash + OS + rustc + target
  │     Restore: exact match
  │
  ├── target/release/        (Gates 6, 7)
  │     Cache key: lockfile hash + OS + rustc + target
  │     Restore: exact match
  │
  └── ~/.cargo/bin/          (Gates 4 — nextest binary)
        Cache key: tool name + version
        Restore: exact match
```

---

## 5. Local Development

### Rust Equivalent of `bin/dev.js`

JS (CLI-MAP §1): `bin/dev.js` imports `dist/bootstrap.js` with `{development: true}`. The `development` flag disables Bugsnag error reporting (CLI-MAP §6.1 step 2: "If not development: registerCleanBugsnagErrorsFromWithinPlugins") and enables verbose debugging.

Rust equivalent: A `dev` feature flag in the `cli` crate:

```toml
# crates/cli/Cargo.toml
[features]
dev = []
```

```rust
// crates/cli/src/main.rs
fn main() {
    if cfg!(feature = "dev") {
        // Enable tracing debug output
        tracing_subscriber::fmt().with_max_level(tracing::Level::DEBUG).init();
        // Skip Bugsnag initialization (CLI-MAP §6.1 step 4 init → "If not development")
        // Enable hot-reload of command registration (for development only)
    } else {
        // Production: structured logging, Bugsnag initialization
        tracing_subscriber::fmt().with_max_level(tracing::Level::INFO).init();
    }
    // Run the CLI
}
```

Development invocation: `cargo run --package cli --features dev -- app dev`

This is the Rust equivalent of `./bin/dev.js shopify app dev`.

### How GraphQL Codegen Runs Locally vs CI

**Both local and CI:** `build.rs` runs `cynic-codegen` on every `cargo build`. There is no separation — the codegen IS the compilation.

**Implication for local dev speed:**
- Cold build: `build.rs` compiles and runs cynic-codegen for each API surface (5+ schemas). Adds ~2–5 seconds per schema to the build.
- Warm build (incremental): `build.rs` is only re-run if the `.graphql` files or `schema.graphql` change. If only `.rs` files changed, `build.rs` is cached. No overhead on the edit-compile loop for non-GraphQL changes.
- Adding a new query: Developer edits a `.graphql` file → recompiles `build.rs` → cynic regenerates → compilation continues. Total overhead ~3–8 seconds on the first build after the change.

**Mitigation:** If `build.rs` becomes a bottleneck, schema files can be pre-compiled and checked in as generated `.rs` files, bypassing `build.rs` for local dev. CI would still run the full codegen to catch drift.

### How a Developer Runs a Single Command

| Task | Command |
|---|---|
| Run `shopify app dev` with production binary | `cargo run --package cli -- app dev` |
| Run with dev features | `cargo run --package cli --features dev -- app dev` |
| Run just the tests | `cargo nextest run` |
| Run tests for one crate | `cargo nextest run -p app` |
| Run a specific test | `cargo nextest run -p app --test deploy_tests` |
| Check compilation without running | `cargo check --workspace` |
| Run clippy on one crate | `cargo clippy -p theme -- -D warnings` |
| Generate shell completions | `cargo run --package cli -- completions bash > shopify.bash` |

### How Environment Variables Are Managed Locally

JS environment helpers from CLI-MAP §5.2 mapped to Rust:

| JS Env Variable | CLI-MAP §5.2 Ref | Rust Equivalent | Purpose |
|---|---|---|---|
| `SHOPIFY_CLI_ENV` | `serviceEnvironment()` | `std::env::var("SHOPIFY_CLI_ENV")` | `"local"` vs `"production"` — controls FQDN resolution (cli-fqdn crate), Bugsnag initialization |
| `SHOPIFY_CLI_PARTNERS_TOKEN` | `getPartnersToken()` env check | `std::env::var("SHOPIFY_CLI_PARTNERS_TOKEN")` | Bypass OAuth for Partners API — used in CI, E2E tests (PORT §4.3 step 1) |
| `SHOPIFY_APP_AUTOMATION_TOKEN` | `getAppAutomationToken()` | `std::env::var("SHOPIFY_APP_AUTOMATION_TOKEN")` | Exchange for App Management + Business Platform tokens (PORT §4.3 step 1-2) |
| `SHOPIFY_CLI_SKIP_NETWORK_LEVEL_RETRY` | `skipNetworkLevelRetry()` | `std::env::var("SHOPIFY_CLI_SKIP_NETWORK_LEVEL_RETRY").map(is_truthy)` | Disable retry for testing |
| `SHOPIFY_CLI_NEVER_USE_PARTNERS_API` | `blockPartnersAccess()` | `std::env::var("SHOPIFY_CLI_NEVER_USE_PARTNERS_API").map(is_truthy)` | Force AppManagementClient (CLI-MAP §6.4) |
| `SHOPIFY_http_proxy` / `SHOPIFY_https_proxy` | `createGlobalProxyAgent()` | `reqwest::Proxy::custom()` reading `http_proxy`/`https_proxy` + custom SHOPIFY_ prefix handling (PORT §7.14) | HTTP proxy support |
| `NO_COLOR` / `TERM` | `forceNoColor()` | `std::env::var("NO_COLOR").is_ok() \|\| std::env::var("TERM").map(\|t\| t == "dumb").unwrap_or(false)` | Disable ANSI color output |
| `DEBUG=*` (verbose) | `setupEnvironmentVariables()` | `RUST_LOG=debug` via `tracing_subscriber` (set by `--verbose` flag or env) | Enable debug tracing |

**Local `.env` file:** The CLI should automatically load `.env` files from the current directory or `~/.shopify-cli/.env` using `dotenvy` crate. This mirrors the JS pattern of reading environment variables from multiple sources (CLI-MAP §6.1 step 4: `loadEnvironment`).

**`isDevelopment()` equivalent (CLI-MAP §5.2):**

```rust
fn is_development() -> bool {
    std::env::var("SHOPIFY_CLI_ENV").map(|v| v == "local").unwrap_or(false)
        || cfg!(feature = "dev")
        || cfg!(debug_assertions) // auto-detect debug build
}
```

---

## 6. Open Questions

Questions that cannot be answered from CLI-MAP.md, API-shopify.md, PORT.md, or TEST-MAP.md alone:

### 6.1 `doctor-release` Check Details
RESOLVED — see §2 Gate 8 above. The actual checks are theme init + push workflow validation (5 assertions across 2 suites). The earlier speculative list (API connectivity checks) was incorrect.

### 6.2 Shell Completion Generation Timing
PORT §6.4 marks `refresh-manifests` as "N/A (no oclif manifest in Rust)." This document proposes generating shell completions via `clap_complete` at release time. However, the JS `refresh-manifests` also produces an oclif README auto-generation. If a Rust equivalent is desired (e.g., auto-generated README with command tables), a separate crate like `clap-markdown` or a custom generator would be needed. The four documents do not specify whether README auto-generation is required.

### 6.3 Binary Installation Path
The proposed install script installs to `~/.shopify-cli/bin/` or `/usr/local/bin/`. The JS CLI is installed via npm to the Node.js global `node_modules/.bin/`. The four documents do not specify where the Rust binary should be installed, what the upgrade mechanism should look like, or whether the upgrade command should self-update (download new binary) or delegate to a package manager.

### 6.4 Cross-Compilation Target Triples
PORT §6.1 specifies `reqwest` with `rustls-tls` (no OpenSSL), enabling musl targets. But the four documents do not explicitly list which target triples to ship. The proposed targets (linux-gnu, linux-musl, macos-intel, macos-arm) are an inference from the dependency choices. A decision on Windows support (x86_64-pc-windows-msvc) is not derivable from the available documents.

### 6.5 Code Signing Infrastructure
macOS release requires Apple Developer Program enrollment for code signing and notarization. The four documents do not specify whether Shopify has the required certificates, what team ID to use, or whether the CI has access to the signing keys.

### 6.6 Install Script Hosting
The proposed `install.sh` script needs to be hosted somewhere accessible. The four documents do not specify whether Shopify maintains a static hosting solution (e.g., `shopify.dev/install.sh`), whether the script is committed to the repo, or whether it's served from GitHub Pages.

### 6.7 Notification System for `upgrade` Command
CLI-MAP §4.8 lists `notifications generate/list` commands and PORT §7.13 flags the notification system as undocumented. The `upgrade` command presumably checks for updates by comparing local version against a remote source. The four documents do not specify: (a) where the remote version is checked (GitHub Releases API? npm registry? custom endpoint?), (b) how often the check runs, (c) how the notification is displayed.

### 6.8 Auto-Upgrade Mechanism
CLI-MAP §4.8 lists `config autoupgrade on/off/status` commands. The four documents do not specify how auto-upgrade works in JS (download new npm package? symlink update?), and therefore cannot specify how it should work in Rust.

### 6.9 CI-specific Test Flag behavior
TEST-MAP §1 documents that E2E tests use Mocha + Chai with custom setup scripts. The exact environment variables, mock server setup, and test data fixtures for the Rust `assert_cmd` E2E tests are not derivable from the available documents — they would need to be extracted from the JS E2E test source files.

### 6.10 Hydrogen Commands
CLI-MAP §3 lists `@shopify/cli-hydrogen` as a dependency of `packages/cli` but the document does not catalog Hydrogen-specific commands (PORT §7.11). The DEVOPS plan cannot determine whether Hydrogen commands require additional build steps, separate binaries, or plugin loading.

### 6.11 Binary Size Budget
The proposed post-processing strips the binary but does not use UPX. If Shopify has a binary size constraint (e.g., for npm-published Rust binaries or embedded environments), the four documents do not specify it.

---

## 7. Human Decisions Required

Three open decisions that must be made by the team before the Rust rewrite plan can be finalized:

### 7.1 Code Signing (from §6.5)

**Question:** Does the team have Apple Developer Program enrollment and signing certificates available in CI secrets?

| Choice | Action Required | Consequence |
|---|---|---|
| **Yes** → | Add `codesign` + `xcrun notarytool` steps to Gate 6 post-processing. Requires `APPLE_DEVELOPER_TEAM_ID`, `APPLE_CERTIFICATE_BASE64`, `APPLE_CERTIFICATE_PASSWORD`, `APPLE_KEYCHAIN_PASSWORD`, `APPLE_NOTARY_KEY_ID`, `APPLE_NOTARY_ISSUER_ID` secrets. | Signed binaries accepted by macOS Gatekeeper. Users install without terminal bypass. |
| **No** → | Ship unsigned binaries. Add `xattr -d com.apple.quarantine` instruction to README install docs. | Users must manually bypass Gatekeeper. Higher friction on macOS. |

### 7.2 Binary Size Budget (from §6.11)

**Question:** Is there a hard size constraint on the binary?

| Choice | Action Required | Consequence |
|---|---|---|
| **Yes, ≤10 MB** → | Evaluate UPX compression, feature-flagging heavy deps (ratatui, reqwest TLS backends), or splitting into sub-binaries. | Slower startup (UPX decompression) or reduced feature set in default binary. |
| **No constraint** → | Strip only. Current 15–25 MB estimate is acceptable. | Fast startup, full feature set. Larger download size. |

### 7.3 README Auto-Generation (from §6.2)

**Question:** Is a machine-generated command reference (like oclif README) required for the Rust rewrite?

| Choice | Action Required | Consequence |
|---|---|---|
| **Yes** → | Add `clap-markdown` crate or custom generator to the release pipeline. Runs before GitHub Release creation, diffs the generated README. | Auto-generated command reference stays in sync with code. Familiar to JS maintainers. |
| **No** → | Document commands manually or rely on `cargo doc` for developer-facing docs. | Lower CI complexity. Manual docs drift without process enforcement. |
