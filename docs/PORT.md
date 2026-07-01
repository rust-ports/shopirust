# PORT.md: Rust Rewrite Plan

## 1. Workspace Layout

### Crate Map

| JS Package (CLI-MAP.md §3) | Rust Crate | Type | Responsibility |
|---|---|---|---|
| `packages/cli-kit` | `cli-kit` | lib | Foundation SDK: API clients (REST/GraphQL), session/auth, output/UI, filesystem, error handling, analytics, caching |
| `packages/cli` | `cli` | bin | Main CLI binary; registers all commands via `clap` subcommands, composes from `app`, `theme`, `store`, `plugin-*` crates |
| `packages/app` | `app` | lib | App development commands: `dev`, `deploy`, `build`, `init`, `generate`, `env`, `config`, `logs`, `function`, `webhook`, bulk operations |
| `packages/theme` | `theme` | lib | Theme commands: `push`, `pull`, `dev`, `delete`, `list`, `info`, `open`, `share`, `check` |
| `packages/create-app` | `create-app` | bin | Standalone `shopify-create` binary; auto-injects `init` subcommand |
| `packages/store` | `store` | lib | Store-level operations |
| `packages/organizations` | `organizations` | lib | Organization-level operations |
| `packages/plugin-cloudflare` | `plugin-cloudflare` | lib | Cloudflare tunnel provider |
| `packages/plugin-did-you-mean` | `plugin-did-you-mean` | lib | Command autocorrection ("did you mean?") |
| *(no JS equivalent)* | `cli-core` | lib | Shared command infrastructure (`BaseCommand` equivalent, environment resolution, hook lifecycle) — extracted from `cli-kit`'s bootstrap + launcher + base-command |
| *(no JS equivalent)* | `cli-api` | lib | DeveloperPlatformClient abstraction + its two implementations (PartnersClient, AppManagementClient) — extracted from `app`'s developer-platform-client.ts |

### Dependency Graph

```
cli-core (bootstrap, launcher, base-command)
  ├── cli-kit (SDK: API clients, session, UI, fs, error, analytics, caching)
  │     ├── app (app commands)
  │     │     ├── cli (main binary, composes all commands)
  │     │     └── create-app (standalone binary)
  │     ├── theme (theme commands)
  │     │     └── cli
  │     ├── store (store commands)
  │     │     └── cli
  │     ├── plugin-cloudflare
  │     │     └── cli
  │     └── plugin-did-you-mean
  │           └── cli
  └── cli-api (DeveloperPlatformClient)
        ├── depends on cli-kit for PartnersClient, AppManagementClient, etc.
        └── used by app, theme, store, organizations
```

### Published vs Internal

| Crate | Published to crates.io? | Notes |
|---|---|---|
| `cli-kit` | Yes | Foundation SDK — could be published as `shopify-cli-kit` |
| `app` | Maybe | Could be extracted as `shopify-app-commands` |
| `theme` | Maybe | Could be extracted as `shopify-theme-commands` |
| `store` | Private | Internal |
| `organizations` | Private | Internal |
| `plugin-cloudflare` | Yes | — |
| `plugin-did-you-mean` | Maybe | — |
| `create-app` | No | Published as npm, not cargo |
| `cli` | No | Final binary artifact |
| `cli-core` | Private | Internal abstraction |
| `cli-api` | Private | Internal abstraction |

---

## 2. Crate Inventory

### `cli-kit` (lib)
- **Key external deps:** `tokio`, `reqwest` (rustls-tls), `cynic`, `serde`+`serde_json`, `thiserror`, `ratatui`, `governor`, `tracing`
- **Why this boundary:** Foundation layer that every other crate depends on. Analogous to the JS `cli-kit` which has zero internal dependencies. Contains all cross-cutting concerns: API transport, auth, UI, caching, error handling, analytics. Any crate that needs to talk to Shopify or render output depends on `cli-kit`. Testing this in isolation ensures API correctness propagates upward.

### `cli-core` (lib)
- **Key external deps:** `clap` (derive), `cli-kit`, `tokio`, `serde`
- **Why this boundary:** JS `base-command.ts` and `cli-launcher.ts` live in `cli-kit` but are conceptually the command framework, not the SDK. Extracting provides a clean point for `BaseCommand`-equivalent trait, environment resolution (development/production), hook lifecycle, error handling orchestration, and the `run_cli` entry point. All command crates (`app`, `theme`, `store`) depend on this.

### `cli-api` (lib)
- **Key external deps:** `cli-kit`, `async-trait`, `serde`
- **Why this boundary:** JS `DeveloperPlatformClient` interface with ~50 methods and its two implementations (`PartnersClient`, `AppManagementClient`) live in the `app` package but are used by `theme`, `store`, and `organizations` too. Extracting prevents a circular dependency or forcing `app`-crate dependency on non-app commands. The trait lives here; implementations call the 6 API surface clients from `cli-kit`.

### `app` (lib)
- **Key external deps:** `cli-kit`, `cli-core`, `cli-api`, `clap`, `tokio`, `serde`, `reqwest` (for dev server/proxy)
- **Why this boundary:** All app-development logic (dev, deploy, build, init, generate, etc.) in one place. The JS `app` package is the largest domain crate. Isolates Shopify-app-specific logic so changes don't affect theme or store commands.

### `theme` (lib)
- **Key external deps:** `cli-kit`, `cli-core`, `clap`, `tokio`, `serde`
- **Why this boundary:** Theme synchronization (push/pull/diff) is a distinct domain with its own file-watching, checksumming, and Admin API call patterns. Does NOT need `cli-api` — it talks directly to the Admin API client.

### `store` (lib)
- **Key external deps:** `cli-kit`, `cli-core`, `clap`, `tokio`
- **Why this boundary:** Store operations are minimal but distinct. Depends on `cli-api` for org/store lookups.

### `organizations` (lib)
- **Key external deps:** `cli-kit`, `cli-core`, `clap`, `tokio`
- **Why this boundary:** Org listing and selection commands. Depends on `cli-api`.

### `plugin-cloudflare` (lib)
- **Key external deps:** `cli-kit`, `cli-core`, `clap`, `tokio`, `reqwest` (for Cloudflare API calls)
- **Why this boundary:** Tunnel provider via Cloudflare's Argo/Quick Tunnel. Swappable if other tunnel providers are needed.

### `plugin-did-you-mean` (lib)
- **Key external deps:** `cli-kit`, `cli-core`, `clap`, `tokio`, `strsim` (for levenshtein distance)
- **Why this boundary:** Command autocorrection independent of any domain. Can be tested with just `clap` command metadata.

### `cli` (bin)
- **Key external deps:** `clap`, `cli-kit`, `cli-core`, `cli-api`, `app`, `theme`, `store`, `plugin-cloudflare`, `plugin-did-you-mean`, `tokio`
- **Why this boundary:** Final binary. Analogous to JS `packages/cli`. Composes subcommands from all domain crates. No business logic — pure wiring.

### `create-app` (bin)
- **Key external deps:** `clap`, `cli-kit`, `cli-core`, `app`, `tokio`
- **Why this boundary:** Standalone binary. Auto-injects `init` subcommand. Minimal surface — reuses `app`'s init logic.

### Additional infrastructure crates derived from helpers in CLI-MAP.md §5:

#### `cli-cache` (lib)
- **JS analog:** `conf-store.ts` (`cacheRetrieveOrRepopulate`)
- **Key deps:** `serde`, `serde_json`, `sha2`, `tokio`, `tracing`
- **Purpose:** TTL-based cache-aside with SHA-256 composite keys. Used by the GraphQL client. Stored on-disk (TODO: decide format — SQLite, sled, or simple JSON file).

#### `cli-retry` (lib)
- **JS analog:** `retryAwareRequest` / `simpleRequestWithDebugLog` / `sleep-with-backoff.ts`
- **Key deps:** `tokio`, `tracing`
- **Purpose:** Exponential backoff retry with jitter, max retry time window, and network-error detection. Wraps `reqwest` calls.

#### `cli-analytics` (lib)
- **JS analog:** `analytics.ts` (Monorail) + `error-handler.ts` (Bugsnag)
- **Key deps:** `serde`, `serde_json`, `reqwest`, `tokio`, `tracing`
- **Purpose:** Flush command analytics events to Shopify's Monorail. Report unexpected errors to error analytics endpoint (`https://error-analytics-production.shopifysvc.com`). Configurable to skip in development/local mode.

#### `cli-fqdn` (lib)
- **JS analog:** `context/fqdn.ts` (`partnersFqdn()`, `appManagementFqdn()`, `businessPlatformFqdn()`, `appDevFqdn()`)
- **Key deps:** `serde`, `tracing`
- **Purpose:** Resolve API FQDNs based on `SHOPIFY_CLI_ENV` (production vs. staging). Each API surface has a deterministic FQDN resolver. Used by `cli-kit`'s API client constructors.

---

## 3. API Surface Clients

All clients live in `cli-kit`. Each wraps the same generic `GraphQLClient` (powered by `reqwest` + `cynic`) and optionally a REST transport.

### 3.1 PartnersClient

- **Struct:** `PartnersClient`
- **Base URL:** `https://{partners_fqdn()}/api/cli/graphql` — FQDN resolved by `cli-fqdn::partners_fqdn()`
- **Auth type:** `BearerToken(String)` — wraps the Partners API access token
- **Rate limiter:** `governor::RateLimiter::<NotKeyed, InMemoryState, QuantaClock>` — quota: `Quota::with_period(Duration::from_millis(150))` (10 requests/second ≈ 1 per 150ms)
- **GraphQL method:** `async fn query<T: cynic::QueryFragment>(&self, operation: cynic::Operation<T>, variables: T::Variables) -> Result<T>` — generic over any `cynic::QueryFragment`, uses `cynic::http::ReqwestAdapter` with the `reqwest` client
- **REST method:** `async fn poll_app_logs(&self, cursor: Option<String>, filters: Option<LogFilter>) -> Result<AppLogsResponse>` — HTTP GET to `https://{partners_fqdn()}/app_logs/poll` with query params `cursor`, `status`, `source`

### 3.2 AdminClient (GraphQL + REST)

- **Struct:** `AdminClient`
- **Base URL (GraphQL):** `https://{store}/admin/api/{version}/graphql.json` — version auto-discovered via `public_api_versions` query, cached per store FQDN. For theme-access sessions (`shptka_` tokens): `https://{theme_kit_access_domain}/cli/admin/api/{version}/graphql.json`
- **Base URL (REST):** `https://{store}/admin/api/{version}{path}.json`
- **Auth type:** `AdminSession { token: String, store_fqdn: String }` — `token` wraps the admin API token (either user-oauth or theme-access token)
- **Rate limiter:** None (no rate limiting in JS either)
- **GraphQL method:** `async fn query<T: cynic::QueryFragment>(&self, operation: cynic::Operation<T>, variables: T::Variables) -> Result<T>` — same shape as PartnersClient but with Admin-specific request headers (`X-Shopify-Shop`, `X-Shopify-Access-Token` for theme access sessions)
- **REST method:** `async fn rest_request<T: DeserializeOwned>(&self, method: Method, path: &str, body: Option<&T>, params: HashMap<String, String>) -> Result<RestResponse<T>>` — constructs full REST URL, serializes body, returns `RestResponse<T> { json, status, headers }`

### 3.3 AppManagementClient

- **Struct:** `AppManagementClient`
- **Base URL:** `https://{app_management_fqdn()}/app_management/unstable/graphql`
- **Auth type:** `BearerToken(String)` — uses the App Management API access token
- **Rate limiter:** Same `governor` config as PartnersClient (150ms min interval)
- **GraphQL method:** Same generic shape as `PartnersClient.query`
- **REST method:** `async fn poll_app_logs(&self, org_id: &str, cursor: Option<String>, filters: Option<LogFilter>) -> Result<AppLogsResponse>` — HTTP GET to `https://{app_management_fqdn()}/app_management/unstable/organizations/{org_id}/app_logs/poll`

### 3.4 AppDevClient

- **Struct:** `AppDevClient`
- **Base URL:** `https://{app_dev_fqdn(store_fqdn)}/app_dev/unstable/graphql.json`
- **Auth type:** `BearerToken(String)` — Partners token
- **Rate limiter:** Same `governor` config as PartnersClient
- **GraphQL method:** Same generic shape. May add `x-forwarded-host` header when `service_environment() == "local"`.

### 3.5 BusinessPlatformClient

- **Struct:** `BusinessPlatformClient`
- **Base URLs:**
  - Destinations: `https://{business_platform_fqdn()}/destinations/api/2020-07/graphql`
  - Organizations: `https://{business_platform_fqdn()}/organizations/api/unstable/organization/{org_id}/graphql`
- **Auth type:** `BearerToken(String)` — Business Platform token
- **Rate limiter:** None (no rate limiting in JS)
- **GraphQL method (Destinations):** Same generic shape
- **GraphQL method (Organizations):** Same generic shape with extra `org_id: String` parameter

### 3.6 WebhooksClient

- **Struct:** `WebhooksClient`
- **Base URL:** `https://{app_management_fqdn()}/webhooks/unstable/organizations/{org_id}/graphql.json`
- **Auth type:** `BearerToken(String)` — App Management token
- **Rate limiter:** Same `governor` config as PartnersClient
- **GraphQL method:** Same generic shape

### 3.7 FunctionsClient

- **Struct:** `FunctionsClient`
- **Base URL:** `https://{app_management_fqdn()}/functions/unstable/organizations/{org_id}/{app_id}/graphql`
- **Auth type:** `BearerToken(String)` — App Management token
- **Rate limiter:** Same `governor` config as PartnersClient
- **GraphQL method:** Same generic shape

### 3.8 OAuthClient (Admin REST)

- **Struct:** `OAuthClient`
- **Base URL:** `https://{store}/admin/oauth/access_token`
- **Auth type:** `ClientCredentials { client_id: String, client_secret: String }`
- **Rate limiter:** None
- **Method:** `async fn exchange_client_credentials(&self, store_fqdn: &str, client_id: &str, client_secret: &str) -> Result<AdminSession>` — `POST` with JSON body `{ client_id, client_secret, grant_type: "client_credentials" }`, returns `AdminSession { token, store_fqdn }`. Handles `app_not_installed` error.

### 3.9 Shared GraphQL Client Core

- **Struct:** `GraphQLClient` (inside `cli-kit`)
- **Responsibility:** Core engine shared by all 7 GraphQL API clients above. Equivalent to JS `graphql.ts`.
- **Capabilities:**
  - HTTP POST to arbitrary URL with headers
  - `cynic`-based query execution via `reqwest` adapter
  - Rate-limit awareness (delegates to `governor` per-client)
  - Retry with backoff (delegates to `cli-retry`)
  - Auto-refresh token on 401 (via `UnauthorizedHandler` trait)
  - Caching (delegates to `cli-cache`)
  - Request timing instrumentation (`tracing` spans → `cli-analytics`)
  - `x-request-id` capture

---

## 4. Auth Layer

All functions live in `cli-kit`'s session module. Return types correspond to token types consumed by API surface clients above.

### 4.1 Function Signatures

| JS function (CLI-MAP.md §4.4) | Rust signature | Returns | Unlocks |
|---|---|---|---|
| `ensureAuthenticatedUser` | `async fn ensure_authenticated_user(env: &EnvMap, options: &AuthOptions) -> Result<UserId>` | `UserId(String)` | (identity only, no API) |
| `ensureAuthenticatedPartners` | `async fn ensure_authenticated_partners(scopes: &[Scope], env: &EnvMap, options: &AuthOptions) -> Result<PartnersToken>` | `PartnersToken(String)` | `PartnersClient`, `AppDevClient` |
| `ensureAuthenticatedAppManagementAndBusinessPlatform` | `async fn ensure_authenticated_app_management_and_business_platform(options: &AuthOptions, app_scopes: &[Scope], bp_scopes: &[Scope], env: &EnvMap) -> Result<AppManagementAndBusinessPlatformTokens>` | `AppManagementAndBusinessPlatformTokens { app_management: AppManagementToken, business_platform: BusinessPlatformToken }` | `AppManagementClient`, `BusinessPlatformClient`, `WebhooksClient`, `FunctionsClient` |
| `ensureAuthenticatedStorefront` | `async fn ensure_authenticated_storefront(scopes: &[Scope], password: Option<&str>, options: &AuthOptions) -> Result<StorefrontToken>` | `StorefrontToken(String)` | Storefront Renderer API |
| `ensureAuthenticatedAdmin` | `async fn ensure_authenticated_admin(store: &str, scopes: &[Scope], options: &AuthOptions) -> Result<AdminSession>` | `AdminSession { token: String, store_fqdn: String }` | `AdminClient` |
| `ensureAuthenticatedThemes` | `async fn ensure_authenticated_themes(store: &str, password: Option<&str>, scopes: &[Scope], options: &AuthOptions) -> Result<AdminSession>` | `AdminSession { token: String, store_fqdn: String }` | `AdminClient` (same type, but token may be `shptka_` theme access) |
| `ensureAuthenticatedBusinessPlatform` | `async fn ensure_authenticated_business_platform(scopes: &[Scope], options: &AuthOptions) -> Result<BusinessPlatformToken>` | `BusinessPlatformToken(String)` | `BusinessPlatformClient` |
| `ensureAuthenticatedAdminAsApp` | `async fn ensure_authenticated_admin_as_app(store_fqdn: &str, client_id: &str, client_secret: &str) -> Result<AdminSession>` | `AdminSession { token: String, store_fqdn: String }` | `AdminClient` (via OAuth client_credentials, not user) |
| `logout` | `async fn logout() -> Result<()>` | `()` | (removes all stored sessions) |

### 4.2 Token Type Hierarchy

```rust
pub enum ApiToken {
    Partners(PartnersToken),
    Admin(AdminSession),
    AppManagement(AppManagementToken),
    BusinessPlatform(BusinessPlatformToken),
    Storefront(StorefrontToken),
}

pub struct AdminSession {
    pub token: String,
    pub store_fqdn: String,
    // When true, request headers become X-Shopify-Shop + X-Shopify-Access-Token
    // instead of Authorization: Bearer
    pub is_theme_access: bool,
}

// All token newtypes:
pub struct PartnersToken(pub String);
pub struct AppManagementToken(pub String);
pub struct BusinessPlatformToken(pub String);
pub struct StorefrontToken(pub String);
```

### 4.3 Auth Flow

JS `ensureAuthenticated` delegates to `private/node/session.ts` which handles the OAuth device authorization flow with Shopify Identity. The Rust equivalent should:
1. Check environment variables first (`SHOPIFY_APP_AUTOMATION_TOKEN`, `SHOPIFY_CLI_PARTNERS_TOKEN`)
2. If env token present, exchange it for appropriate API tokens (via `exchange_custom_partner_token`, `exchange_app_automation_token_for_app_management`, etc.)
3. Otherwise, read stored tokens from secure storage (keychain or encrypted file)
4. If no stored tokens, initiate OAuth device authorization flow (open browser, poll for token)
5. Return typed token

**Note:** CLI-MAP.md documents the interface but not the full OAuth flow internals. The exact OAuth endpoints and token exchange mechanics would need to be extracted from the JS `private/node/session/` and `private/node/session/exchange.ts` files, which are not covered in the available documents.

---

## 5. Command Structure

### 5.1 Commands from `packages/cli` (crate: `cli`)

These are the top-level commands registered in the binary:

| clap subcommand | Crate owning implementation | DeveloperPlatformClient method(s) called |
|---|---|---|
| `version` | `cli` (inline) | None |
| `upgrade` | `cli` (inline) | None |
| `search` | `cli` (inline) | None |
| `help` | `cli` (inline) | None (uses clap's built-in) |
| `auth login` | `cli` (inline) | None (triggers OAuth flow) |
| `auth logout` | `cli` (inline) | None (calls `logout()`) |
| `cache clear` | `cli` (inline) | None (calls `clear_cache()`) |
| `config autoupgrade on/off/status` | `cli` (inline) | None |
| `debug command-flags` | `cli` (inline) | None |
| `docs generate` | `cli` (inline) | None |
| `doctor-release` | `cli` (inline) | Multiple (org lookup, app lookup) |
| `kitchen-sink *` | `cli` (inline) | None (UI component demo) |
| `notifications generate/list` | `cli` (inline) | None |

### 5.2 Commands from `packages/app` (crate: `app`)

| clap subcommand | DeveloperPlatformClient method(s) called |
|---|---|
| `app dev` | `ensureAuthenticatedPartners`, `ensureAuthenticatedAdmin`, `orgAndApps`, `appExtensionRegistrations`, `appFromIdentifiers`, `specifications`, `devSessionCreate`, `devSessionUpdate`, `devSessionDelete`, `generateSignedUploadUrl`, `deploy`, `updateURLs`, `activeAppVersion` |
| `app deploy` | `orgAndApps`, `appExtensionRegistrations`, `appFromIdentifiers`, `specifications`, `generateSignedUploadUrl`, `deploy`, `release` |
| `app build` | None (local only) |
| `app init` | `createApp`, `organizations`, `orgAndApps` |
| `app info` | `appFromIdentifiers` (may also call `activeAppVersion`, `appVersions`) |
| `app logs` | `subscribeToAppLogs`, `appLogs` |
| `app release` | `release`, `appVersionsDiff` |
| `app execute` | `generateSignedUploadUrl`, `deploy` (bulk) |
| `app import-extensions` | `appExtensionRegistrations` |
| `app config link` | `appFromIdentifiers`, `updateURLs` |
| `app config pull` | `appFromIdentifiers`, `activeAppVersion` |
| `app config use` | None (local file switch) |
| `app config validate` | None (local validation) |
| `app env pull` | None (local file) |
| `app env show` | None |
| `app function build` | None (local build) |
| `app function run` | None (local runner) |
| `app function replay` | `appFromIdentifiers` |
| `app function schema` | `targetSchemaDefinition` or `apiSchemaDefinition` |
| `app function typegen` | `targetSchemaDefinition` or `apiSchemaDefinition` |
| `app function info` | `appFromIdentifiers` |
| `app generate extension` | `specifications`, `templateSpecifications`, `createExtension` |
| `app versions list` | `appVersions` |
| `app webhook trigger` | `sendSampleWebhook`, `apiVersions`, `topics` |
| `app bulk cancel` | `appFromIdentifiers` |
| `app bulk execute` | `appFromIdentifiers`, `generateSignedUploadUrl` |
| `app bulk status` | `appFromIdentifiers` |
| `app dev clean` | None |

### 5.3 Commands from `packages/theme` (crate: `theme`)

| clap subcommand | DeveloperPlatformClient method(s) called |
|---|---|
| `theme push` | `ensureAuthenticatedThemes` (calls AdminClient directly for theme CRUD) |
| `theme pull` | `ensureAuthenticatedThemes` (calls AdminClient directly) |
| `theme dev` | `ensureAuthenticatedThemes` (calls AdminClient directly) |
| `theme delete` | `ensureAuthenticatedThemes` (calls AdminClient directly) |
| `theme list` | `ensureAuthenticatedThemes` (calls AdminClient directly) |
| `theme info` | `ensureAuthenticatedThemes` (calls AdminClient directly) |
| `theme open` | None |
| `theme share` | `ensureAuthenticatedThemes` (calls `themeCreate` + `themePublish`) |
| `theme check` | None (local analysis) |

### 5.4 Commands from `packages/store` (crate: `store`)

| clap subcommand | DeveloperPlatformClient method(s) called |
|---|---|
| `store create` | `organizations`, `devStoresForOrg`, `convertToTransferDisabledStore`, `ensureUserAccessToStore` |

### 5.5 Command dispatch logic (CLI-MAP.md §6.4)

The `DeveloperPlatformClient` trait is resolved at runtime:
- If the org's `source == BusinessPlatform` → `AppManagementClient`
- Else if `first_party_dev() && !block_partners_access()` → `PartnersClient`
- Else → `AppManagementClient`

This means `app deploy`, `app dev`, `app release`, and all other commands that call `DeveloperPlatformClient` methods must be written against the trait (interface), not a concrete client. The `SelectDeveloperPlatformClient` equivalent should be injected into the command via constructor or as a parameter.

Theme commands are the exception — they always use `AdminClient` directly and never go through `DeveloperPlatformClient`.

### 5.6 Command structure pattern

```rust
// In app crate
#[derive(clap::Args)]
pub struct DevCommand {
    /// Store URL to develop against
    #[arg(long, short)]
    pub store: Option<String>,

    // ... more flags
}

impl DevCommand {
    pub async fn run(&self, client: &dyn DeveloperPlatformClient) -> Result<()> {
        // 1. Resolve the developer platform client for the org
        // 2. Call client methods
        // 3. Render UI via cli-kit's ratatui helpers
    }
}

// In cli crate
#[derive(clap::Subcommand)]
pub enum AppCommand {
    Dev(DevCommand),
    Deploy(DeployCommand),
    // ...
}
```

---

## 6. Build Pipeline

### 6.1 Workspace Cargo.toml Structure

```toml
# /Cargo.toml (workspace root)
[workspace]
resolver = "2"
members = [
    "crates/cli-kit",
    "crates/cli-core",
    "crates/cli-api",
    "crates/cli-cache",
    "crates/cli-retry",
    "crates/cli-analytics",
    "crates/cli-fqdn",
    "crates/app",
    "crates/theme",
    "crates/store",
    "crates/organizations",
    "crates/plugin-cloudflare",
    "crates/plugin-did-you-mean",
    "crates/cli",
    "crates/create-app",
]

[workspace.dependencies]
tokio = { version = "1", features = ["full"] }
clap = { version = "4", features = ["derive"] }
reqwest = { version = "0.12", default-features = false, features = ["rustls-tls", "json"] }
cynic = { version = "3", features = ["http-reqwest-rustls", "json"] }
cynic-codegen = "3"
ratatui = "0.28"
thiserror = "2"
anyhow = "1"
governor = "0.7"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
tracing = "0.1"
tracing-subscriber = "0.3"

# ... other deps
```

### 6.2 GraphQL Codegen

JS approach: `.graphql` files → `@graphql-codegen/*` → generated TypeScript types.

Rust approach: `.graphql` files → `cynic-codegen` (`build.rs`).

Each API surface has its own schema and set of query/mutation files:

```
crates/cli-kit/
  api/
    partners/
      schema.graphql          # Partners API schema (fetched at build time)
      queries/
        find_org.graphql
        find_app.graphql
        all_app_extension_registrations.graphql
        create_app.graphql
        # ... etc.
      mutations/
        app_deploy.graphql
        app_release.graphql
        extension_create.graphql
        # ... etc.
    admin/
      schema.graphql          # Admin API schema
      queries/
        get_themes.graphql
        get_theme_file_bodies.graphql
        public_api_versions.graphql
        # ... etc.
      mutations/
        theme_create.graphql
        theme_files_upsert.graphql
        # ... etc.
    app_management/
      schema.graphql
      queries/
      mutations/
    # ... etc. for each API surface
```

`build.rs` in each crate that has `.graphql` files:
```rust
fn main() {
    cynic_codegen::register_schema("partners")
        .from_schema_file("api/partners/schema.graphql")
        .unwrap();
    // Repeat for each schema
}
```

Generated code is re-exported as typed `cynic::QueryFragment` structs, consumed by the generic `GraphQLClient::query()` method.

### 6.3 Single Binary Output

JS: `esbuild bundle` → single .js file per package.

Rust: `cargo build --package cli --release` → single `shopify` binary in `target/release/`.

The `cli` crate depends on all domain crates (`app`, `theme`, `store`, etc.) and registers their clap subcommands via composition. No bundling step needed — Rust's static linking produces a single portable binary.

The `create-app` crate produces a separate `shopify-create` binary.

### 6.4 CI / Task Pipeline

JS Nx pipeline:
```
build -> ^build
bundle -> build -> other_crate_builds
type-check -> ^build
lint -> (parallel)
```

Rust CI equivalent:

| JS target | Rust equivalent |
|---|---|
| `build` | `cargo build` (or `cargo check` for faster CI) |
| `bundle` | `cargo build --release` |
| `type-check` | `cargo check` (inherent in the compiler) |
| `lint` | `cargo clippy --all-targets -- -D warnings` |
| `lint:fix` | `cargo clippy --fix` |
| `test` | `cargo nextest run` (or `cargo test`) |
| `graphql-codegen` | `cargo build` (build.rs runs cynic-codegen automatically) |
| `refresh-manifests` | N/A (no oclif manifest in Rust) |
| `clean` | `cargo clean` |

### 6.5 Caching Strategy

JS uses Nx caching with SHA-based content hashing.

Rust equivalent: `sccache` (or `mold` + incremental compilation). GitHub Actions `actions/cache` for `~/.cargo` and `target/` directories. `cargo-chef` for Docker layer caching if containerized builds.

---

## 7. Open Questions

### 7.1 OAuth Device Authorization Flow
The auth layer's `ensure_authenticated_*` functions are documented at the interface level (CLI-MAP.md §4.4), but the actual OAuth device authorization flow (redirect URI, token polling, token refresh, identity token exchange) is in JS `private/node/session/` and `private/node/session/exchange.ts` files which are not covered by available docs. The Rust implementation will need to reverse-engineer these files to determine:
- The OAuth authorize/token endpoints and their parameters
- The device code polling loop logic
- Token refresh mechanics
- Identity token → API token exchange protocol
- How Business Platform and App Management tokens are minted from the same identity session

### 7.2 Extension Build Pipeline
CLI-MAP.md §4.7 lists `app build`, `app function build`, and `app dev` but does not document the actual build steps (Webpack/Vite/esbuild integration for UI extensions, wasm compilation for functions). The Rust `app` crate will need to spawn these external build tools as subprocesses. The exact CLI flags and config files passed to these tools need extraction from JS `services/build/`.

### 7.3 Theme File Body Parsing
API-shopify.md documents that theme files can return bodies of type `TEXT`, `BASE64`, or `URL`. The `URL` type requires fetching an external URL to get the content. This is documented at the function level (`parseThemeFileContent`) but the URL-fetching retry logic and download behavior are not fully specified. The Rust AdminClient needs to handle this in the response deserialization layer.

### 7.4 Bulk Operations Polling
The JS bulk operations service polls `query getBulkOperationById` until completion. The polling interval, timeout, and cancellation mechanics are not documented in API-shopify.md. The Rust equivalent needs to replicate the polling logic with configurable backoff.

### 7.5 Dev Server / File Watcher
`app dev` involves a local HTTPS dev server (CLI-MAP.md §4.7 mentions "hot-reload"), file watching for extension changes, WebSocket connections, and proxy logic. This is a significant subsystem not covered in available docs. The Rust implementation may need to spawn Node.js/Vite subprocesses for the actual HMR server while the CLI manages the lifecycle and proxy configuration.

### 7.6 Theme Check Language Server
The `theme check` command invokes a language server. The JS implementation uses `@shopify/theme-check-node`. The Rust equivalent would need to either reimplement the Liquid analysis or spawn the Node.js checker as a subprocess.

### 7.7 Tunnel Providers
`plugin-cloudflare` provides Cloudflare tunnel integration. API-shopify.md does not cover the Cloudflare API endpoints used to create/manage tunnels. This would need extraction from the JS `plugin-cloudflare` source files.

### 7.8 Conf-store Implementation Details
CLI-MAP.md mentions `conf-store.ts` provides `cacheRetrieveOrRepopulate` for TTL-based caching, but the storage backend (JSON file? SQLite? Encrypted?) is not specified. The Rust `cli-cache` crate needs to decide on a storage backend. The documents also mention `ConfSchema` with a `GraphQLRequestKey` type — the exact cache key schema and stored data format need extraction.

### 7.9 Analytics Endpoint Protocol
CLI-MAP.md §6.1 mentions `reportAnalyticsEvent() (Monorail)` but does not specify the Monorail REST endpoint, request format, or batching behavior. The Rust `cli-analytics` crate needs this protocol information from the JS `analytics.ts` source files.

### 7.10 Node.js Version Check
CLI-MAP.md §6.1 step 2 mentions `exitIfOldNodeVersion` (checks Node >= 18). In Rust there is no Node version to check. This step can be omitted unless the Rust binary replaces a Node runtime that downstream tools depend on.

### 7.11 Hydrogen Commands
CLI-MAP.md §3 package table lists `@shopify/cli-hydrogen` as a dependency of `packages/cli`, but the document does not catalog Hydrogen-specific commands or their DeveloperPlatformClient usage. Hydrogen commands are skipped in this plan until their source files are analyzed.

### 7.12 Environment File Override
CLI-MAP.md §6.1 step 4 mentions `loadEnvironment` which reads environment configuration files and merges them into parsed flags. The exact file format, lookup path algorithm, and merge priority rules are not documented. The Rust `cli-core` equivalent needs this specification.

### 7.13 Notification System
CLI-MAP.md §4.8 lists `notifications generate/list` commands and CLI-MAP.md §6.1 step 4 calls `showNotificationsIfNeeded()` during command init. The notification system's data source, format, and display rules are not documented.

### 7.14 Proxy Agent Configuration
CLI-MAP.md §6.1 step 2 calls `createGlobalProxyAgent()` which sets up HTTP proxy support via `SHOPIFY_`-namespaced environment variables. The Rust `reqwest` client has built-in proxy support via `Proxy::custom()` that reads `http_proxy`/`https_proxy` env vars. The SHOPIFY-specific namespace may need custom handling.
