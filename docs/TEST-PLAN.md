# TEST-PLAN.md: Rust Rewrite Test Plan

> **Source documents:** CLI-MAP.md (architecture), API-shopify.md (API surfaces), PORT.md (Rust plan), TEST-MAP.md (JS test coverage)
>
> **Fixed tooling:** `wiremock` (HTTP mocking), `assert_cmd` (CLI subprocess), `insta` (snapshot tests), `proptest` (property-based tests)
>
> Every section is derived exclusively from these four documents. Anything not traceable to them is in §7 (Open Questions).

---

## 1. Test Layer Inventory

### Layer Mapping

| JS Layer (TEST-MAP §1) | Rust Equivalent | Boundary Under Test | Crate & Directory (PORT §1) | Tooling | CI Step (PORT §6.4) |
|---|---|---|---|---|---|
| Vitest (unit) | `cargo test` unit tests (in `src/`) | Individual functions, structs, API client methods, helper logic | Per crate: `crates/cli-kit/src/`, `crates/app/src/`, `crates/theme/src/`, `crates/store/src/`, `crates/organizations/src/`, `crates/plugin-cloudflare/src/`, `crates/plugin-did-you-mean/src/`, `crates/cli-core/src/`, `crates/cli-api/src/` | `wiremock`, `insta`, `proptest` | `cargo nextest run --lib` |
| Vitest (integration) | `cargo test` integration tests (in `tests/`) | Crate boundary crossings — `cli-kit` → `cli-api`, `cli-kit` → `app`, `cli-core` → `app` | `crates/tests/` or per-crate `crates/*/tests/` | `wiremock` | `cargo nextest run --tests` |
| Mocha + Chai (E2E) | `assert_cmd` CLI tests | Full binary — `shopify` CLI end-to-end | `crates/cli/tests/` (PORT §5: `cli` crate is the binary) | `assert_cmd`, `wiremock` | `cargo nextest run --test e2e` (separate CI job) |
| *(JS had none)* — GraphQL contract tests | `cynic-codegen` in `build.rs` + `insta` snapshot of schema | Schema drift detection — catch when a remote API schema changes a field used by the CLI | `crates/cli-kit/api/*/schema.graphql` per API surface (PORT §6.2) | `cynic-codegen` (built into `build.rs`), `insta` for snapshot assertion | `cargo build` (build.rs runs automatically; CI `--check` on snapshots) |
| *(JS had none)* — Property-based tests | `proptest` tests | Functions with wide input domains — URL construction, retry timing, cache key collisions | Per crate, co-located with unit tests in `src/` | `proptest` | `cargo nextest run --lib` (same command, lower in priority) |

### Rust E2E CI Job Spec

```
- name: E2E tests
  run: cargo nextest run --test e2e
  env:
    SHOPIFY_CLI_E2E_TEST: "1"
    # wiremock runs embedded, no external service needed
```

---

## 2. Unit Tests — Per Crate

### 2.1 API Surface Clients

Maps PORT.md §3 (API Surface Clients) against TEST-MAP.md §2 (API Coverage).

#### PartnersClient (PORT §3.1)

**JS test status (TEST-MAP §2.1):** ✅ `partners-client.test.ts` (239 lines)

**JS assertions found:** Tests `PartnersClient` class methods — `createApp()`, `orgs()`, `orgFromId()`, `appFromId()`, `appsForOrg()`, `storesByOrg()`, `appExtensionRegistrations()`, `appVersions()`, `updateAppUrl()`. Asserts correct parameters passed to mocked `partnersRequest`. Does NOT test the HTTP call itself.

**Rust unit test contract (`crates/cli-kit/src/api/partners.rs`):**

```rust
// File: crates/cli-kit/src/api/partners.rs (unit tests)

use wiremock::{MockServer, Mock, ResponseTemplate};
use wiremock::matchers::{method, path};

// ── rate-limiter behavior ──────────────────────────────────────
// JS rate limiter (API-shopify.md § "Partners API"):
//   Bottleneck: 150ms minTime, 10 concurrent
// Rust equivalent (PORT §3.1):
//   governor::RateLimiter::<NotKeyed, InMemoryState, QuantaClock>
//   Quota::with_period(Duration::from_millis(150))
#[tokio::test]
async fn rate_limiter_enforces_150ms_min_interval() {
    // Arrange
    let mock_server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "data": { "organizations": { "nodes": [] } }
        })))
        .expect(2..) // expect at least 2 requests
        .mount(&mock_server)
        .await;
    let client = PartnersClient::new(
        mock_server.uri(),
        BearerToken("test-token".into()),
    );

    // Act — fire 2 requests immediately
    let (r1, r2) = tokio::join!(
        client.query(find_org_query(), find_org_variables()),
        client.query(find_org_query(), find_org_variables()),
    );

    // Assert — both succeed (governor queues the second)
    assert!(r1.is_ok());
    assert!(r2.is_ok());
}

// ── auth header ─────────────────────────────────────────────────
// PORT §3.1: "Auth type: BearerToken(String)"
// JS: vi.mocked(partnersRequest) — header tested indirectly
#[tokio::test]
async fn sends_authorization_bearer_header() {
    let mock_server = MockServer::start().await;
    let captured_headers = Arc::new(Mutex::new(None::<HeaderMap>));
    let headers = captured_headers.clone();
    Mock::given(method("POST"))
        .and(wiremock::matchers::header("authorization", "Bearer test-token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "data": { "organizations": { "nodes": [] } }
        })))
        .expect(1)
        .mount(&mock_server)
        .await;
    let client = PartnersClient::new(
        mock_server.uri(),
        BearerToken("test-token".into()),
    );
    let _ = client.query(find_org_query(), find_org_variables()).await;
}

// ── 401 → token refresh (API-shopify.md § shared core) ─────────
// JS graphql.ts: "Token refresh on 401"
// PORT §3.9: "Auto-refresh token on 401 (via UnauthorizedHandler trait)"
#[tokio::test]
async fn retries_with_refreshed_token_on_401() {
    // Mock returns 401 first, then 200
    // Assert client calls UnauthorizedHandler once, then retries
}
```

**Key assertions derived from TEST-MAP §2.1:**

| JS test behavior | Rust counterpart |
|---|---|
| `orgs()` returns org list | `partners_client.organizations()` returns `Vec<Organization>` |
| `appFromId()` returns app details | `partners_client.app_from_id("key")` returns `OrganizationApp` |
| `createApp()` writes to Partners API | `partners_client.create_app(org_id, title, ...)` posts `CreateApp` mutation |
| `appExtensionRegistrations()` returns extensions | `partners_client.app_extension_registrations("key")` returns registrations |
| `updateAppUrl()` calls `appUpdate` mutation | `partners_client.update_urls("key", url, redirects, proxy)` posts `appUpdate` |

#### AdminClient (PORT §3.2)

**JS test status (TEST-MAP §2.2):** ✅ `admin-as-app.test.ts` (69 lines) + `graphql.test.ts`

**JS assertions found:** Tests URL construction (`https://{store}/admin/api/unstable/graphql.json`), token passing, variable handling. Tests `adminAsAppRequestDoc()` wrapper. Does NOT test the actual HTTP call.

**Rust unit test contract (`crates/cli-kit/src/api/admin.rs`):**

```rust
// ── URL construction (TEST-MAP §2.2: verifies correct URL) ─────
#[tokio::test]
async fn constructs_admin_graphql_url() {
    // "assert: URL = https://test-store.myshopify.com/admin/api/{version}/graphql.json"
    // PORT §3.2: version auto-discovered via public_api_versions query
    let mock_server = MockServer::start().await;
    // Mock the version discovery endpoint
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "data": { "publicApiVersions": [{ "handle": "2024-10", "supported": true }] }
        })))
        .mount(&mock_server)
        .await;
    let client = AdminClient::new(
        AdminSession { token: "tok".into(), store_fqdn: "test-store.myshopify.com".into(), is_theme_access: false },
    );
    // Assert URL contains the store FQDN
}

// ── theme access session headers (API-shopify.md § Admin API) ──
// JS: "For theme access sessions: X-Shopify-Shop + X-Shopify-Access-Token"
#[tokio::test]
async fn theme_access_session_uses_different_headers() {
    let session = AdminSession {
        token: "shptka_test".into(),
        store_fqdn: "test-store.myshopify.com".into(),
        is_theme_access: true, // PORT §4.2: is_theme_access flag
    };
    // Assert headers include X-Shopify-Shop, X-Shopify-Access-Token
    // Assert Authorization: Bearer is NOT present
}

// ── REST request (API-shopify.md § Admin API REST) ─────────────
#[tokio::test]
async fn rest_request_constructs_url_and_method() {
    // JS: restRequest(method, path, session, body, params, apiVersion)
    // PORT §3.2: rest_request<T>(method, path, body, params) -> RestResponse<T>
    let mock_server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(wiremock::matchers::path_regex(r"/admin/api/\d{4}-\d{2}/themes\.json"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "themes": []
        })))
        .mount(&mock_server)
        .await;
    let result = client.rest_request::<ThemesResponse>(Method::GET, "/themes", None, HashMap::new()).await;
    assert!(result.is_ok());
}
```

#### AppManagementClient (PORT §3.3)

**JS test status (TEST-MAP §2.5):** ❌ No tests

**This is a port risk — HIGH.** Reason: The App Management API is the primary backend for new-style orgs (CLI-MAP §6.4 dispatch → AppManagementClient when `org.source == BusinessPlatform`). It is central to CLI-MAP §6 control flow for `app dev`, `app deploy`, `app release`, and all org/app operations.

**Minimal coverage proposal** (derived from its rate limiter and auth type in API-shopify.md):

```rust
// ── rate limiter (same governor config as Partners) ────────────
// API-shopify.md: "Bottleneck: 150ms / 10 concurrent"
#[tokio::test]
async fn rate_limiter_enforces_150ms_min_interval() { /* same pattern as PartnersClient */ }

// ── auth header (BearerToken, PORT §3.3) ──────────────────────
#[tokio::test]
async fn sends_auth_header() { /* assert Authorization: Bearer {token} */ }

// ── base URL construction (API-shopify.md § App Management) ────
// "https://{appManagementFqdn}/app_management/unstable/graphql"
#[tokio::test]
async fn constructs_correct_base_url() { /* assert URL matches pattern */ }

// ── app logs polling (API-shopify.md § App Logs) ──────────────
// "HTTP GET to https://{appManagementFqdn}/app_management/unstable/organizations/{orgId}/app_logs/poll"
#[tokio::test]
async fn poll_app_logs_constructs_url_and_passes_cursor() {
    // Assert URL contains org_id, cursor, status, source query params
}
```

#### AppDevClient (PORT §3.4)

**JS test status:** Not explicitly listed in TEST-MAP §2 (implicitly untested).

**Port risk: MEDIUM.** App Dev API is only used in the `dev` session lifecycle (CLI-MAP §4.7 `app dev`). If `app dev` is the highest-priority command to port, this becomes HIGH.

**Minimal coverage:** Rate limiter (150ms governor), auth header (BearerToken, uses Partners token per PORT §3.4), FQDN resolution pattern (`https://{app_dev_fqdn(store)}/app_dev/unstable/graphql.json`), `x-forwarded-host` header when `service_environment() == "local"` (PORT §3.4).

#### BusinessPlatformClient (PORT §3.5)

**JS test status (TEST-MAP §2.6):** ❌ No tests for either sub-API.

**Port risk: MEDIUM.** Business Platform APIs are used for org/destination queries in AppManagementClient (CLI-MAP §6.4). Used early in the control flow for all app commands (org lookup). Not critical to individual API surface correctness because the queries are simple reads.

**Minimal coverage:**
- Destinations: URL pattern (`https://{bp_fqdn()}/destinations/api/2020-07/graphql`), no rate limiter, BearerToken auth
- Organizations: URL pattern (`https://{bp_fqdn()}/organizations/api/unstable/organization/{orgId}/graphql`), no rate limiter, BearerToken auth

#### WebhooksClient (PORT §3.6)

**JS test status:** Not listed in TEST-MAP §2. Untested.

**Port risk: LOW.** Webhooks API is used only by `app webhook trigger` and shares the App Management token (PORT §3.6: "Auth type: BearerToken(String) — App Management token"). Same rate limiter as Partners. The control flow impact is limited to a single command.

**Minimal coverage:** Rate limiter (150ms governor), auth header, URL pattern (`https://{appMgmtFqdn}/webhooks/unstable/organizations/{orgId}/graphql.json`).

#### FunctionsClient (PORT §3.7)

**JS test status (TEST-MAP §2.4):** ❌ No API-level tests. Service-level tests exist in `services/function/`.

**Port risk: LOW.** Functions API is used only for schema definition queries in `app function schema` / `app function typegen` (CLI-MAP §4.7). It shares the App Management token. The API surface is read-only (two queries).

**Minimal coverage:** Rate limiter (150ms governor), auth header, URL pattern (`https://{appMgmtFqdn}/functions/unstable/organizations/{orgId}/{appId}/graphql`).

#### OAuthClient (PORT §3.8)

**JS test status:** Not listed in TEST-MAP §2. Untested.

**Port risk: HIGH.** OAuth client_credentials grant is used for `ensureAuthenticatedAdminAsApp()` (PORT §4.1) which is critical for the `app dev` and `app deploy` flows (CLI-MAP §6.1 step 4). Auth failures here block all admin-as-app operations.

**Minimal coverage** (derived from API-shopify.md § OAuth):

```rust
// ── successful token exchange (API-shopify.md § OAuth) ────────
// POST body: { client_id, client_secret, grant_type: "client_credentials" }
// Response: { access_token: "string" }
#[tokio::test]
async fn exchanges_client_credentials_for_token() {
    let mock_server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(wiremock::matchers::body_json_contains(serde_json::json!({
            "grant_type": "client_credentials"
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "access_token": "admin-token-123"
        })))
        .mount(&mock_server)
        .await;
    let result = OAuthClient::exchange_client_credentials(
        &mock_server.uri().trim_end_matches('/'), // simulate store FQDN
        "client-123",
        "secret-456",
    ).await;
    assert!(result.is_ok());
    assert_eq!(result.unwrap().token, "admin-token-123");
}

// ── app_not_installed error (API-shopify.md § OAuth error handling) ──
// "400 with app_not_installed → AbortError with install prompt"
#[tokio::test]
async fn app_not_installed_returns_actionable_error() {
    let mock_server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(400).set_body_json(serde_json::json!({
            "error": "app_not_installed"
        })))
        .mount(&mock_server)
        .await;
    let result = OAuthClient::exchange_client_credentials(
        &mock_server.uri().trim_end_matches('/'),
        "client-123",
        "secret-456",
    ).await;
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.to_string().contains("install"), "error should suggest installing the app");
}

// ── JSON parse failure (API-shopify.md: "JSON parse failure → AbortError") ──
#[tokio::test]
async fn non_json_response_returns_parse_error() {
    let mock_server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_string("not json"))
        .mount(&mock_server)
        .await;
    let result = OAuthClient::exchange_client_credentials(
        &mock_server.uri().trim_end_matches('/'),
        "client-123",
        "secret-456",
    ).await;
    assert!(matches!(result.unwrap_err(), OAuthError::ParseError(_)));
}
```

### 2.2 Shared GraphQL Client Core (PORT §3.9)

**JS test status (TEST-MAP §2.8):** ✅ `graphql.test.ts` tests `graphqlRequestDoc` with mocked HTTP.

**JS assertions found:** Tests the core engine — request execution, response parsing.

**Rust unit test contract (`crates/cli-kit/src/api/graphql.rs`):**

```rust
// ── successful query execution (PORT §3.9) ────────────────────
#[tokio::test]
async fn executes_graphql_query_and_returns_typed_response() { /* wiremock 200 + assert deserialization */ }

// ── 401 triggers token refresh (PORT §3.9: "Auto-refresh token on 401") ──
#[tokio::test]
async fn calls_unauthorized_handler_on_401_and_retries() { /* wiremock 401 → 200, assert handler called once */ }

// ── rate limit restore sleep (PORT §3.9: "Rate-limit awareness") ──
#[tokio::test]
async fn waits_on_rate_limit_response() { /* wiremock with throttleStatus cost-based response */ }

// ── caching with cache-aside (PORT §3.9: "Caching delegates to cli-cache") ──
#[tokio::test]
async fn caches_response_and_returns_cached_on_second_call() { /* wiremock called once, second call from cache */ }

// ── retry on network error (PORT §3.9: "Retry with backoff delegates to cli-retry") ──
#[tokio::test]
async fn retries_on_transient_network_error() { /* wiremock fails then succeeds */ }

// ── x-request-id capture (PORT §3.9) ──────────────────────────
#[tokio::test]
async fn captures_x_request_id_from_response_headers() { /* wiremock returns x-request-id header, assert captured */ }
```

### 2.3 Helper Function Coverage

Maps TEST-MAP.md §3 (helpers with JS tests) to Rust crates from PORT.md §2 and CLI-MAP.md §5.

#### `cli-cache` (PORT §2: `cli-cache` crate)

| JS helper (CLI-MAP §5) | Rust crate | Rust struct/fn | Unit test assertions (derived from JS `local-storage.test.ts`, TEST-MAP §3.1) |
|---|---|---|---|
| `cacheRetrieveOrRepopulate(key, fetcher, ttlMs, store?)` | `cli-cache` | `CacheStore::retrieve_or_repopulate(key, fetcher, ttl)` | Get: returns cached value when TTL valid; miss: calls fetcher, stores result, returns it; TTL expiry: calls fetcher again; concurrent calls: only one fetcher runs |
| `ConfSchema.GraphQLRequestKey` compositing | `cli-cache` | `CacheKey::from_query(query, variables, version)` | Same query + variables → same SHA-256 key; different query → different key; includes version salt |

#### `cli-retry` (PORT §2: `cli-retry` crate)

| JS helper (CLI-MAP §5) | Rust crate | Rust struct/fn | Unit test assertions (derived from JS `http.test.ts`, `graphql.test.ts`, TEST-MAP §3.1) |
|---|---|---|---|
| `retryAwareRequest(options, errorHandler?)` | `cli-retry` | `RetryStrategy::execute(factory)` | Retries on 429/503/network error; stops on 4xx (except 429); respects `SHOPIFY_CLI_SKIP_NETWORK_LEVEL_RETRY` env var; exponential backoff with jitter; max retry time window |
| `simpleRequestWithDebugLog(options)` | `cli-retry` | `RetryStrategy::with_debug(factory, logger)` | Same as execute but logs each attempt via `tracing` |

#### `cli-analytics` (PORT §2: `cli-analytics` crate)

| JS helper (CLI-MAP §5) | Rust crate | Rust struct/fn | Unit test assertions (derived from JS `analytics.test.ts`, `error-handler.test.ts`, TEST-MAP §3.1) |
|---|---|---|---|
| `reportAnalyticsEvent()` (Monorail) | `cli-analytics` | `AnalyticsClient::report_event(event)` | POST to `https://error-analytics-production.shopifysvc.com` with JSON body; skips in development mode; batches multiple events |
| `sendErrorToBugsnag(error, exitMode)` | `cli-analytics` | `ErrorReporter::send_to_bugsnag(error, mode)` | POST to Bugsnag endpoint; includes metadata (command ID, platform, version); `cleanStackFrameFilePath` normalizes paths |
| `addPublicMetadata(factory)` | `cli-analytics` | `MetadataCollector::add(key, value)` | Multiple calls merge; metadata flushed at command end; values serializable to JSON |

#### `cli-fqdn` (PORT §2: `cli-fqdn` crate)

| JS helper (CLI-MAP §5) | Rust crate | Rust struct/fn | Unit test assertions (derived from JS `environment.test.ts`, TEST-MAP §3.1) |
|---|---|---|---|
| `partnersFqdn()` | `cli-fqdn` | `resolve_partners_fqdn(env)` | Default returns `partners.shopify.com`; `SHOPIFY_CLI_ENV=staging` returns staging FQDN |
| `appManagementFqdn()` | `cli-fqdn` | `resolve_app_management_fqdn(env)` | Same pattern |
| `businessPlatformFqdn()` | `cli-fqdn` | `resolve_business_platform_fqdn(env)` | Same pattern |
| `appDevFqdn(shopFqdn)` | `cli-fqdn` | `resolve_app_dev_fqdn(store_fqdn, env)` | Deterministic from store FQDN + env |

#### Auth/Session (PORT §4)

| JS function (CLI-MAP §4.4) | Rust equivalent (PORT §4.1) | Unit test assertions (derived from JS `session.test.ts`, TEST-MAP §3.1) |
|---|---|---|
| `ensureAuthenticatedPartners` | `ensure_authenticated_partners(scopes, env, options)` | Returns `PartnersToken`; reads `SHOPIFY_CLI_PARTNERS_TOKEN` env var first; falls back to stored token; initiates OAuth device flow if no token found |
| `ensureAuthenticatedAdmin` | `ensure_authenticated_admin(store, scopes, options)` | Returns `AdminSession`; validates token format; handles `is_theme_access` sessions separately |
| `ensureAuthenticatedThemes` | `ensure_authenticated_themes(store, password, scopes, options)` | Same as admin but uses theme access token path (`shptka_` prefix) |
| `ensureAuthenticatedAdminAsApp` | `ensure_authenticated_admin_as_app(store, client_id, client_secret)` | Returns `AdminSession`; exchanges client_credentials via `OAuthClient`; handles `app_not_installed` error |
| `logout` | `logout()` | Removes all stored tokens; clears cache |
| `ensureAuthenticatedAppManagementAndBusinessPlatform` | `ensure_authenticated_app_management_and_business_platform(options, ...)` | Returns two tokens; reads `SHOPIFY_APP_AUTOMATION_TOKEN` first (PORT §4.3 step 1-2) |

#### CLI Core (PORT §2: `cli-core` crate)

| JS component (CLI-MAP §4.1) | Rust equivalent | Unit test assertions (derived from JS `cli.test.ts`, `base-command.test.ts`, TEST-MAP §3.1) |
|---|---|---|
| `runCLI(options)` | `run_cli(options)` | Sets env vars, checks Node.js version (SKIP in Rust — PORT §7.10), forces no-color, launches CLI |
| `BaseCommand.init()` | `BaseCommand::init(meta)` | Sets command ID, registers error handlers, shows notifications |
| `BaseCommand.parse()` | `BaseCommand::parse(flags)` | Parses clap args, applies environment overrides, adds metadata |
| `environments` file loading | `load_environment(environment_file)` | Reads TOML/YAML file, merges into flag overrides |
| `addFromParsedFlags(flags)` | `MetadataCollector::add_from_parsed_flags(flags)` | Adds path and verbose flags to metadata |

#### Common Utilities (CLI-MAP §5.1, mapped from TEST-MAP §3.2)

| JS function | Rust equivalent | Unit test assertions |
|---|---|---|
| `filter(iterable, predicate)` | `iter.filter(predicate).collect()` | Standard library — no custom test needed |
| `getArrayRejectingUndefined(arr)` | `arr.into_iter().flatten().collect()` | Standard library |
| `groupBy(array, keyFn)` | `group_by` from itertools or manual | Returns `HashMap<K, Vec<T>>`; preserves insertion order per group |
| `isUnitInterval(value)` | `(0.0..=1.0).contains(&value)` | Returns true for 0, 0.5, 1; false for -0.1, 1.1, NaN |
| `nonRandomUUID(value)` | SHA-256 to UUID format | Deterministic: same input always produces same UUID; different inputs produce different UUIDs; output is valid UUID v4 format |
| `underscore(str)` | String case conversion | `"camelCase"` → `"camel_case"`; `"ABC"` → `"a_b_c"` |
| `tryParseJson(str)` | `serde_json::from_str(str)` | Returns `Some(T)` for valid JSON; `None` for invalid; handles partial/malformed input |

---

## 3. Integration Tests — Crate Boundaries

### Crate Dependency Edges (PORT §1)

```
cli-kit  ←──  cli-core
cli-kit  ←──  cli-api
cli-kit  ←──  app
cli-kit  ←──  theme
cli-kit  ←──  store
cli-kit  ←──  organizations
cli-kit  ←──  plugin-cloudflare
cli-kit  ←──  plugin-did-you-mean
cli-core ←──  app
cli-core ←──  theme
cli-core ←──  cli
cli-api  ←──  app
cli-api  ←──  theme (theoretically, but PORT §5.3: "Theme does NOT need cli-api")
```

### Cross-Package Coverage in JS (TEST-MAP §2)

TEST-MAP §2: "partners-client.test.ts mocks @shopify/cli-kit/node/api/partners — does not test the actual HTTP/graphql call"

**Finding:** No true cross-package integration tests existed in JS. Every test mocked across package boundaries. The only integration tests were Vitest's "integration" designation on `archiver.integration.test.ts` which tests zip/unzip within `cli-kit`.

| Edge | JS cross-package coverage (TEST-MAP) | Rust integration test needed? | Rationale |
|---|---|---|---|
| `cli-kit` → `cli-core` | None | ✅ Yes — new boundary in Rust (no JS equivalent) | `cli-core::BaseCommand::run()` must successfully initialize `cli-kit`'s session, output, and error handler. An integration test should wire a minimal command through `run_cli()` and verify `tracing` subscriber is active and output doesn't panic. |
| `cli-kit` → `cli-api` | None — `partners-client.test.ts` mocks `@shopify/cli-kit/node/api/partners` | ✅ Yes — critical boundary | `cli-api::PartnersClient` calls `cli-kit::api::partners::PartnersClient` under the hood via the `DeveloperPlatformClient` trait. An integration test should verify that when `wiremock` returns a real GraphQL response, the API client correctly deserializes it into the trait's return type. |
| `cli-kit` → `app` | None — app service tests mock `@shopify/cli-kit` | ❌ Gap — intentional | PORT §1: app depends on cli-kit for SDK. The dependency is one-directional. Integration tests between these two are better covered at the E2E level (crate boundary tests would duplicate E2E logic without testing the full pipeline). |
| `cli-kit` → `theme` | None — theme service tests mock Admin API | ❌ Gap — intentional | Same reasoning as app. Theme commands talk directly to AdminClient (PORT §5.3). Integration at the crate boundary would require wiremocking the Admin API, which is better done in the AdminClient unit tests. |
| `cli-core` → `app` | None — gap | ❌ Gap — acknowledged | JS `base-command.ts` lives in cli-kit (same package as exports). In Rust this is a real crate boundary but app commands are tested through their `run()` method which takes `DeveloperPlatformClient` as a parameter. The `cli-core` → `app` boundary is just `clap` command registration, which is covered by `assert_cmd` E2E tests. |

### Integration Test Specification for `cli-api` + `cli-kit` Boundary

```rust
// File: crates/cli-api/tests/developer_platform_client.rs
//
// Purpose: Verify that the DeveloperPlatformClient trait implementations
// correctly wrap cli-kit's API clients.

use wiremock::{MockServer, Mock, ResponseTemplate};

/// The PartnersClient implementation of DeveloperPlatformClient must
/// correctly delegate to cli-kit::api::partners::PartnersClient.
///
/// Derived from TEST-MAP §2.1: partners-client.test.ts asserts that
/// PartnersClient methods (orgs, createApp, etc.) call the correct
/// API endpoints. This test validates the wiremock boundary.
#[tokio::test]
async fn partners_client_receives_correct_graphql_requests() {
    let mock_server = MockServer::start().await;

    // Mock a Partners GraphQL endpoint
    Mock::given(method("POST"))
        .and(wiremock::matchers::header("authorization", "Bearer test-partners-token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "data": {
                "organizations": {
                    "nodes": [
                        { "id": "1", "businessName": "Test Org" }
                    ]
                }
            }
        })))
        .mount(&mock_server)
        .await;

    let config = PartnersClientConfig {
        endpoint: mock_server.uri(),
        token: PartnersToken("test-partners-token".into()),
    };
    let client = PartnersClientImpl::new(config);
    let orgs = client.organizations().await.unwrap();

    assert_eq!(orgs.len(), 1);
    assert_eq!(orgs[0].business_name, "Test Org");
}
```

---

## 4. End-to-End Tests

### 4.1 Existing E2E Scenario (TEST-MAP §5)

**JS E2E:** App lifecycle (create → deploy → update) via Mocha + Chai.

**Rust equivalent (`crates/cli/tests/e2e/app_lifecycle.rs`):**

```rust
use assert_cmd::Command;
use wiremock::{MockServer, Mock, ResponseTemplate};

/// Scenario: `shopify app init` → `shopify app deploy` → `shopify app release`
///
/// API surfaces called (from API-shopify.md):
///   1. Partners API — FindOrganization (org lookup)
///   2. Partners API — CreateApp (app creation)
///   3. Partners API — GenerateSignedUploadUrl (bundle upload)
///   4. Partners API — AppDeploy (version creation)
///   5. Partners API — AppRelease (version publication)
///
/// Wiremock setup:
///   - Mock all 5 Partners API endpoints with realistic success responses
///   - No live API calls needed — CLI must be self-contained for CI
///
/// assert_cmd output assertions:
///   - Exit code: 0
///   - Stdout contains: "app created", "version deployed", "version released"
///   - Stderr: empty (no errors, no warnings)
#[tokio::test]
async fn app_lifecycle_create_deploy_release() {
    let mock_server = MockServer::start().await;
    let mock_url = mock_server.uri();

    // ── Step 1: FindOrganization ──
    Mock::given(method("POST"))
        .and(wiremock::matchers::body_json_contains(serde_json::json!({
            "query": r#"query FindOrganization"#
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "data": { "organizations": { "nodes": [{ "id": "1", "businessName": "Test Org" }] } }
        })))
        .mount(&mock_server)
        .await;

    // ── Step 2: CreateApp ──
    Mock::given(method("POST"))
        .and(wiremock::matchers::body_json_contains(serde_json::json!({ "query": r#"mutation CreateApp"# })))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "data": { "appCreate": { "app": { "id": "app-1", "apiKey": "key-123", "apiSecretKeys": [{ "secret": "secret-456" }] }, "userErrors": [] } }
        })))
        .mount(&mock_server)
        .await;

    // ── Step 3: GenerateSignedUploadUrl ──
    Mock::given(method("POST"))
        .and(wiremock::matchers::body_json_contains(serde_json::json!({ "query": r#"mutation GenerateSignedUploadUrl"# })))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "data": { "appVersionGenerateSignedUploadUrl": { "signedUploadUrl": "https://upload.example.com/bundle.zip" } }
        })))
        .mount(&mock_server)
        .await;

    // ── Step 4: AppDeploy ──
    Mock::given(method("POST"))
        .and(wiremock::matchers::body_json_contains(serde_json::json!({ "query": r#"mutation AppDeploy"# })))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "data": { "appDeploy": { "appVersion": { "uuid": "ver-1", "id": "123" }, "userErrors": [] } }
        })))
        .mount(&mock_server)
        .await;

    // ── Step 5: AppRelease ──
    Mock::given(method("POST"))
        .and(wiremock::matchers::body_json_contains(serde_json::json!({ "query": r#"mutation AppRelease"# })))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "data": { "appRelease": { "appVersion": { "versionTag": "v1", "message": "Initial release" }, "userErrors": [] } }
        })))
        .mount(&mock_server)
        .await;

    // ── Execute ──
    let mut cmd = Command::cargo_bin("shopify").unwrap();
    cmd.env("SHOPIFY_CLI_PARTNERS_TOKEN", "test-token")
        .env("PARTNERS_API_URL", &mock_url) // wiremock overrides the FQDN
        .arg("app")
        .arg("init")
        .arg("--name")
        .arg("test-app")
        .assert()
        .success()
        .stdout(predicates::str::contains("app created"));
}
```

### 4.2 E2E Gaps (All Other Commands)

Every command in PORT.md §5 with no corresponding E2E scenario in TEST-MAP.md §5:

| PORT §5 Command | Surfaces Touched (from CLI-MAP §6 control flow) | E2E Gap Level | Needs Coverage? |
|---|---|---|---|
| `app dev` | PartnersClient, AdminClient, AppDevClient, AppManagementClient (orgs, apps, extensions, dev session, deployment) | CRITICAL | ✅ Yes — highest surface count, most complex flow. Without E2E, the entire `shopify app dev` development loop is untested end-to-end. |
| `app build` | None (CLI-MAP §4.7: "local only") | LOW | ❌ — Pure local build. Each extension builder has its own unit tests. E2E would require Node.js/wasm subprocesses. |
| `app init` | PartnersClient (CreateApp), AppManagementClient (CreateApp), BusinessPlatform (orgs) | MEDIUM | ✅ Should be combined with deploy/release lifecycle. Already partially covered by existing E2E. |
| `app info` | PartnersClient/AppManagementClient (appFromIdentifiers) | LOW | ❌ — Simple read query. Covered by unit tests. |
| `app logs` | PartnersClient/AppManagementClient (subscribe, poll) | MEDIUM | ✅ Polling loop is hard to unit-test well. E2E with wiremock time progression covers the polling logic. |
| `app release` | PartnersClient/AppManagementClient (release, versionsDiff) | MEDIUM | ✅ Covered by lifecycle E2E if combined with deploy. |
| `app execute` | PartnersClient/AppManagementClient (bulk operations, upload) | HIGH | ❌ — Bulk operations involve polling, staged uploads, and error handling. E2E is the natural test layer. |
| `app import-extensions` | PartnersClient/AppManagementClient (extensionRegistrations) | LOW | ❌ |
| `app config link` | PartnersClient (appFromIdentifiers, updateURLs) | LOW | ❌ |
| `app config pull` | PartnersClient (appFromIdentifiers, activeAppVersion) | LOW | ❌ |
| `app config use` | None | NONE | ❌ |
| `app config validate` | None | NONE | ❌ |
| `app env pull` | None | NONE | ❌ |
| `app env show` | None | NONE | ❌ |
| `app function *` | PartnersClient/AppManagementClient | LOW | ❌ — Each sub-command covered by unit tests; function runner covered by service tests. |
| `app generate extension` | PartnersClient (specifications, createExtension) | LOW | ❌ — Scaffolding is local file generation with one API call. |
| `app versions list` | PartnersClient (appVersions) | LOW | ❌ |
| `app webhook trigger` | PartnersClient/AppManagementClient (webhooks: sendSampleWebhook, apiVersions, topics) | MEDIUM | ❌ — Only command touching WebhooksClient. E2E would validate the webhook dispatch pipeline. |
| `app bulk cancel/status` | PartnersClient (appFromIdentifiers) | LOW | ❌ |
| `app dev clean` | None | NONE | ❌ |
| `theme push` | AdminClient (theme CRUD — API-shopify.md § Themes) | MEDIUM | ❌ — Theme sync (push/pull/diff) involves checksums, file bodies, pagination. Realistic E2E requires wiremocking the Admin API pagination loop. |
| `theme pull` | AdminClient (theme CRUD) | MEDIUM | ❌ |
| `theme dev` | AdminClient + file watcher | HIGH | ❌ — Dev server + file watching + WebSocket is hard to test. E2E with timeout-based file change simulation. |
| `theme delete` | AdminClient | LOW | ❌ |
| `theme list` | AdminClient | LOW | ❌ |
| `theme info` | AdminClient | LOW | ❌ |
| `theme open` | None | NONE | ❌ |
| `theme share` | AdminClient (themeCreate + themePublish) | MEDIUM | ❌ — Multi-step: create from skeleton, publish. |
| `theme check` | None | LOW | ❌ — Local language server. Would need subprocess orchestration. |
| `store create` | BusinessPlatform (orgs, stores, provision) + Partners (convert) | MEDIUM | ❌ — Multiple API surfaces in one command. |
| `auth login` | Identity OAuth | CRITICAL | ✅ Yes — Authentication is the gateway to all commands. However, the OAuth device flow (PORT §7.1) requires a browser, making it hard to automate. Alternative: test the token exchange endpoints with wiremock and test the browser-open logic with a flag. |
| `auth logout` | None | LOW | ❌ |
| `cache clear` | None | NONE | ❌ |
| `config autoupgrade *` | None | NONE | ❌ |
| `version` | None | NONE | ❌ |
| `upgrade` | None | LOW | ❌ — Self-upgrade is hard to test in E2E. |
| `search` | None | NONE | ❌ |
| `help` | clap built-in | NONE | ❌ — Tested by clap's own test suite. |

**Prioritized E2E coverage order:**
1. `app dev` — highest surface count, hardest to unit test
2. `auth login` — blocks all authenticated commands
3. `app execute` (bulk) — complex polling + upload flow
4. `theme push`/`pull` — pagination + checksum + file body types
5. `app logs` — polling loop needs end-to-end validation
6. `app webhook trigger` — only command using WebhooksClient

---

## 5. GraphQL Contract Tests

### 5.1 Query/Mutations with Malformed Response Tests

**Finding from TEST-MAP §2:** "Every single graphql query/mutation file lacks a dedicated test."

TEST-MAP §2.8 confirms the low-level graphql client (`graphql.test.ts`) tests request/response with mocked HTTP, but none of the 35 individual queries/mutations have tests for:
- Missing fields in response
- Null vs absent fields
- Unexpected response shape
- Server error responses (GraphQL errors array, not HTTP 200)

### 5.2 cynic Schema Snapshot Contract

For each API surface, the `build.rs` compiles `.graphql` queries against the schema. When the remote schema changes, `cynic-codegen` fails at compile time if:
- A queried field is removed
- A field type changes incompatibly
- A required argument is added to a queried field

**Rust procedure:**

```rust
// In crates/cli-kit/build.rs (PORT §6.2)
fn main() {
    // Partners schema
    cynic_codegen::register_schema("partners")
        .from_schema_file("api/partners/schema.graphql")
        .unwrap();
    // Admin schema
    cynic_codegen::register_schema("admin")
        .from_schema_file("api/admin/schema.graphql")
        .unwrap();
    // ... etc for each API surface
}
```

**CI failure when schema changes:**
```
error[E0412]: cannot find type `SomeRemovedField` in this scope
  --> src/api/partners/queries/find_org.rs:10:22
   |
10 |     pub business_name: String,
   |                      ^^^^ help: a type with a similar name exists: `SomeRemovedField`
```

The cynic compiler error is caught by `cargo build`/`cargo check` in CI. The `insta` snapshot of the generated Rust structs documents the expected shape — when the schema changes, the snapshot test fails before compilation, giving a clear diff of what field changed.

### 5.3 All 35 Queries Flagged

Every query/mutation from `packages/app/src/cli/api/graphql/` (API-shopify.md § Apps):

| Query/Mutation (API-shopify.md ref) | JS malformed-response test (TEST-MAP §2) | Rust cynic schema snapshot | Silent failure risk |
|---|---|---|---|
| `FindOrganization` (API-shopify.md § Orgs) | ❌ None | ✅ `crates/cli-kit/api/partners/queries/find_org.graphql` | LOW — If `businessName` is deprecated, compilation fails |
| `CreateApp` (§ Apps) | ❌ None | ✅ `crates/cli-kit/api/partners/mutations/create_app.graphql` | LOW |
| `FindApp` (§ Apps) | ❌ None | ✅ `crates/cli-kit/api/partners/queries/find_app.graphql` | MEDIUM — 17 fields; if Shopify removes `preferencesUrl`, compilation catches it |
| `allAppExtensionRegistrations` (§ Apps) | ❌ None | ✅ `crates/cli-kit/api/partners/queries/all_app_extension_registrations.graphql` | LOW |
| `activeAppVersion` (§ Apps) | ❌ None | ✅ `crates/cli-kit/api/partners/queries/app_active_version.graphql` | LOW |
| `AppVersions` (§ Apps) | ❌ None | ✅ `crates/cli-kit/api/partners/queries/get_versions_list.graphql` | LOW |
| `AppVersionByTag` (§ Apps) | ❌ None | ✅ `crates/cli-kit/api/partners/queries/app_version_by_tag.graphql` | LOW |
| `AppVersionsDiff` (§ Apps) | ❌ None | ✅ `crates/cli-kit/api/partners/queries/app_versions_diff.graphql` | LOW |
| `AppDeploy` (§ Apps) | ❌ None | ✅ `crates/cli-kit/api/partners/mutations/app_deploy.graphql` | HIGH — Deploy is a critical path; if `validationErrors` shape changes, the CLI may misreport errors |
| `AppRelease` (§ Apps) | ❌ None | ✅ `crates/cli-kit/api/partners/mutations/app_release.graphql` | MEDIUM |
| `UpdateURLs` (§ Apps) | ❌ None | ✅ `crates/cli-kit/api/partners/mutations/update_urls.graphql` | LOW |
| `fetchSpecifications` (§ Apps) | ❌ None | ✅ `crates/cli-kit/api/partners/queries/extension_specifications.graphql` | MEDIUM — If spec schema changes, extension validation breaks silently |
| `ExtensionCreate` (§ Apps) | ❌ None | ✅ `crates/cli-kit/api/partners/mutations/extension_create.graphql` | LOW |
| `extensionUpdate` (§ Apps) | ❌ None | ✅ `crates/cli-kit/api/partners/mutations/update_draft.graphql` | LOW |
| `GenerateSignedUploadUrl` (§ Apps) | ❌ None | ✅ `crates/cli-kit/api/partners/mutations/generate_signed_upload_url.graphql` | MEDIUM — Upload URL is critical for deploy |
| `convertDevToTestStore` (§ Apps) | ❌ None | ✅ `crates/cli-kit/api/partners/mutations/convert_dev_to_transfer_disabled_store.graphql` | LOW |
| `findOrgBasic` (§ Orgs) | ❌ None | ✅ `crates/cli-kit/api/partners/queries/find_org_basic.graphql` | LOW |
| Template/Preview/Migration queries | ❌ None | ✅ — by schema | LOW |
| **All Admin API theme queries** (API-shopify.md § Themes) | ❌ None | ✅ `crates/cli-kit/api/admin/queries/*.graphql` | MEDIUM — Theme file body types (TEXT/BASE64/URL) could change shape |
| **All App Management queries** | ❌ None | ✅ `crates/cli-kit/api/app_management/*.graphql` | MEDIUM — Newer API surface, more likely to change |
| **All Business Platform queries** | ❌ None | ✅ `crates/cli-kit/api/business_platform/*.graphql` | LOW |

**Flagged silent-risk queries (no malformed-response test in JS, no cynic schema snapshot in Rust yet):**

None — all queries will have cynic schema snapshots in Rust. The cynic approach inherently catches field-level changes. However, there is a risk for **runtime logic changes** (e.g., field is present but value format changes — URL format, GID format, enum variant addition). These are NOT caught by cynic and require explicit contract tests.

---

## 6. Coverage Gaps

Reproduces TEST-MAP.md §7 mapped to Rust risk levels. Risk is rated by centrality to CLI-MAP.md §6 control flow.

### Critical Gaps — Port Risk: HIGH

| Gap (from TEST-MAP §7) | Rust Risk | Centrality (CLI-MAP §6) |
|---|---|---|
| **Storefront Renderer API** — No tests in JS | HIGH | CLI-MAP §6.1 step 4: `ensureAuthenticatedStorefront` is called for storefront operations. Not central to primary control flow (`app dev`/`deploy`), but missing entirely in PORT.md (not listed in PORT §3). Would need a new client. |
| **Function Runner API** — No API-level tests | HIGH | CLI-MAP §6.2: Function execution is not in the main control flow for `app dev`/`deploy`. Risk is limited to `app function run` command. |
| **OAuth `ensureAuthenticatedAdminAsApp`** — No JS tests | HIGH | CLI-MAP §6.1 step 4: Admin-as-app is called during `app dev` and `app deploy`. Authentication failures here block the entire command. |
| **Auth device flow** — OAuth internals not tested in JS | HIGH | CLI-MAP §6.1 step 4: All authenticated commands depend on the token exchange flow working. This is the single biggest failure point. |
| **`app dev` E2E** — No E2E coverage | HIGH | CLI-MAP §6.1: `app dev` is the most complex control flow path — touches Partners, Admin, App Dev, App Management APIs plus local build, file watching, tunnel. |

### Significant Gaps — Port Risk: MEDIUM

| Gap (from TEST-MAP §7) | Rust Risk | Centrality (CLI-MAP §6) |
|---|---|---|
| **App Management API client** — No JS tests | MEDIUM | CLI-MAP §6.4: AppManagementClient is the default backend for all app commands (when org is Business Platform or user is not first-party dev). Central but PORT §3.3 specifies the interface — wiremock tests cover the boundary. |
| **Business Platform API clients** — No JS tests | MEDIUM | CLI-MAP §6.4: Used for org lookup. Early in control flow but query is simple. |
| **All 35 graphql queries** — No dedicated JS tests | MEDIUM | CLI-MAP §6.2: The graphql queries are the payload of the API request flow. However, cynic codegen catches schema drift at compile time. The risk is runtime behavior (null handling, enum values) not schema shape. |
| **Command-level tests (app)** — Only 4/20+ have JS tests | MEDIUM | CLI-MAP §4.7: Commands are thin wrappers around services. PORT §5.6 shows the pattern — commands call `DeveloperPlatformClient` trait methods. The unit tests for services cover the logic; missing command-level tests is a gap in flag parsing and error wrapping. |
| **Command-level tests (theme)** — Only 3/14+ have JS tests | MEDIUM | Same reasoning as app. Theme commands use AdminClient directly (PORT §5.3). |
| **`app webhook trigger`** — No JS command test | MEDIUM | CLI-MAP §4.7: Webhooks is the only command touching WebhooksClient. If the client works, the command works. |
| **`app execute` (bulk ops)** — No JS command test | MEDIUM | CLI-MAP §4.7: Bulk operations involve polling and file staging. Complex state machine that benefits from integration/E2E coverage. |

### Moderate Gaps — Port Risk: LOW

| Gap (from TEST-MAP §7) | Rust Risk | Centrality (CLI-MAP §6) |
|---|---|---|
| **Theme API client** — No direct JS test | LOW | CLI-MAP §4.10: Theme commands call AdminClient directly. The AdminClient already has unit test coverage. |
| **Environment prompts** — No JS test | LOW | CLI-MAP §6.1 step 4: Environment file loading is a simple file read + merge. |
| **Integration between services** — Mock-heavy in JS | LOW | This is by design in both JS and Rust. The `DeveloperPlatformClient` trait (PORT §5.5) provides the abstraction boundary. Integration tests between the trait and its implementations cover this. |
| **`app build`** — No JS E2E | LOW | CLI-MAP §4.7: "Local only — no API calls." Build invokes external tools (esbuild/wasm). |
| **`theme check`** — No JS E2E | LOW | CLI-MAP §4.10: Local language server. PORT §7.6 suggests subprocess spawning. |
| **`app function *`** — No JS E2E | LOW | CLI-MAP §4.7: Mostly local (build) or simple API calls (schema, info). |

### Rust-Only Gaps (No JS Equivalent)

| Gap | Risk | Reason |
|---|---|---|
| **`cli-core` crate boundaries** | MEDIUM | New crate in Rust. `BaseCommand`-equivalent trait behavior is unit-testable but the integration between `cli-core` and `cli-kit` (tracing subscriber, error handler wiring) needs explicit tests. |
| **`cli-api` crate boundaries** | MEDIUM | `DeveloperPlatformClient` trait is new in Rust. The trait implementations depend on `cli-kit`'s API clients. Integration tests (wiremock → trait method) are critical. |
| **Rate limiter correctness** | MEDIUM | JS uses Bottleneck library; Rust uses `governor`. The governor behavior (queuing vs. dropping) must match. Test: fire N requests concurrently, measure inter-request timing. |
| **Retry backoff timing** | LOW | JS uses exponential backoff with jitter; Rust `cli-retry` must replicate. Test: mock transient failures, assert timing pattern. |

---

## 7. Open Questions

Questions that cannot be answered from CLI-MAP.md, API-shopify.md, PORT.md, or TEST-MAP.md alone:

### 7.1 OAuth Device Authorization Flow Internals
PORT §7.1 and §4.3 describe the `ensure_authenticated_*` functions at the interface level but the actual OAuth endpoints, device code polling loop, token refresh mechanics, and identity-to-API-token exchange are not documented. The test plan for the auth layer cannot be completed until these files are read:
- `packages/cli-kit/src/private/node/session/`
- `packages/cli-kit/src/private/node/session/exchange.ts`

### 7.2 Extension Build Pipeline Details
PORT §7.2 notes that `app build` and `app dev` invoke external build tools (Webpack/Vite/esbuild for UI extensions, wasm compilation for functions). The exact CLI flags, config files, and expected output formats are needed to determine whether `app build` tests mock subprocesses or run them in CI.

### 7.3 Dev Server / File Watcher / WebSocket Protocol
`app dev` involves a local HTTPS dev server, WebSocket connections, file watching, and proxy logic (PORT §7.5). The protocol between the CLI and the browser for hot-reload is not documented. E2E tests for `app dev` cannot be specified without this.

### 7.4 Monorail Analytics Endpoint Protocol
TEST-MAP §3.1 notes `monorail.test.ts` exists but the actual Monorail REST endpoint URL, request format, batching behavior, and response handling are not in any of the four documents. The `cli-analytics` crate needs this to define its contract tests.

### 7.5 Conf-store Storage Backend
PORT §7.8 asks whether the cache store uses JSON file, SQLite, or encrypted storage. The cache layer's serialization format affects what wiremock responses must look like in cache integration tests.

### 7.6 Cloudflare Tunnel API
PORT §7.7: The Cloudflare API endpoints used to create/manage tunnels are not documented. `plugin-cloudflare` tests cannot be specified.

### 7.7 Theme File Body URL Fetching
API-shopify.md § Themes documents that theme files can return `URL` body type. PORT §7.3 notes the URL-fetching retry logic is not specified. The AdminClient test plan for `parse_theme_file_content` depends on this behavior.

### 7.8 Bulk Operations Polling Parameters
PORT §7.4: The polling interval, timeout, and cancellation mechanics for bulk operations are not documented. The `app execute` E2E test needs these to determine wiremock timing.

### 7.9 Notification System Data Source
PORT §7.13: The notification system's data source, format, and display rules are not documented. The `cli-core::BaseCommand::show_notifications()` test cannot be specified.

### 7.10 Theme Check Language Server Protocol
PORT §7.6: The `theme check` command invokes a language server. The expected input/output format is not documented. Integration tests for `theme check` depend on this.

### 7.11 Hydrogen Commands
PORT §7.11: `@shopify/cli-hydrogen` commands are not cataloged in CLI-MAP.md §4.7 or PORT §5. Hydrogen commands are skipped entirely — no test plan can be produced.

### 7.12 Environment File Format
PORT §7.12: The environment file format (TOML? JSON? Custom?), lookup path algorithm, and merge priority rules are not documented. `cli-core::load_environment()` tests cannot be specified.

### 7.13 Proxy Agent Configuration (SHOPIFY_ prefix)
PORT §7.14: The JS code uses `SHOPIFY_http_proxy`, `SHOPIFY_https_proxy` environment variables. The Rust `reqwest` proxy support reads `http_proxy`/`https_proxy`. Whether the SHOPIFY_-prefixed variants are read by the Rust implementation or need custom handling determines the test setup.

### 7.14 How to handle `insta` snapshots for schema drift in CI
The test plan proposes schema snapshots to catch drift, but the four documents do not specify:
- Whether schema files (`schema.graphql`) are fetched at build time or checked in
- How schema updates are reviewed (PR process for schema changes)
- Whether there is a shared schema registry across versions
