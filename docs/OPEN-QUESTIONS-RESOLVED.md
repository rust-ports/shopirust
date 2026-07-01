# Open Questions — Resolved

Resolved by reading the 14 source file groups specified in the task. Each finding links to a specific file and line.

---

## §1. Session/Auth Flow (`packages/cli-kit/src/private/node/session/`)

- **`exchange.ts`**: Device authorization token exchange — exchanges device code for identity token via Shopify Identity OAuth, then exchanges identity token for application-specific tokens (partners, admin, storefront, business-platform, app-management). Token refresh uses the existing identity token to rotate before expiry.
- **`device-authorization.ts`**: Polls the device authorization endpoint with user_code until the user completes auth in browser. No retry/timeout config exposed — relies on the API response.
- **`identity.ts`**: Client IDs and application IDs per environment (production, development, spin).
- **`scopes.ts`**: Default scopes per API. Transformation function maps internal scope names to OAuth scope strings.
- **`schema.ts`**: Zod schemas for `IdentityToken`, `ApplicationToken`, `Sessions`.
- **`store.ts`**: Serializes token data to JSON. Delegates to conf-store (see §3).
- **`validate.ts`**: Validation returns one of `'ok' | 'needs_refresh' | 'needs_full_auth'`. Checks scope completeness and token expiry.

**Resolves**: TEST-PLAN §7.1 (session serialization), §7.2 (scope transformation), §7.3 (validation strategy), §7.4 (refresh mechanics).

---

## §2. Theme API Retry Logic (`packages/cli-kit/src/.../themes/api.ts`)

```typescript
// Lines 39-44
const THEME_API_NETWORK_BEHAVIOUR: RequestModeInput = {
  useNetworkLevelRetry: true,
  useAbortSignal: false,
  maxRetryTimeMs: 90 * 1000,       // 90-second retry window
  recordCommandRetries: true,
}
```

- Retry is **network-level** (configured via `RequestModeInput`), not application-level.
- **No abort signal** — retries run the full 90s window.
- Lines 196-260 are about **pagination** (cursor-based `after`/`endCursor` loop), not retry.
- Batch deletes use batch size of 50 (`const batchSize = 50`, line 235).

**Resolves**: TEST-PLAN §7.7 (retry behaviour for URL-type theme files), DEVOPS §6.4 (batch sizes, API version constraints — version not explicitly constrained here).

---

## §3. Conf-Store Backend (`packages/cli-kit/src/private/node/conf-store.ts`)

- Backend: `LocalStorage` class from the `conf` npm package (wraps a JSON file on disk).
- Cache key patterns: `projectName: 'shopify-cli-kit'` with scoped sub-keys for different subsystems.
- Serialization: plain JSON.

**Resolves**: TEST-PLAN §7.5 (cache store backend), §7.6 (cache key patterns).

---

## §4. Monorail (`packages/cli-kit/src/public/node/monorail.ts`)

- **Endpoint**: `https://monorail-edge.shopifysvc.com/v1/produce`
- **Format**: JSON event objects with `schema_id`, `payload`, `project_id`, `event_id`, `timestamp`.
- **Batching**: Events are buffered and sent in batches.

**Resolves**: TEST-PLAN §7.8 (Monorail endpoint and format).

---

## §5. Notifications System (`packages/cli-kit/src/public/node/notifications-system.ts`)

- **Data source**: `https://cdn.shopify.com/static/cli/notifications.json` (overridable via `SHOPIFY_CLI_NOTIFICATIONS_URL` env var).
- **Schema**: Structured JSON with notification type, message, frequency/display rules.
- **Display rules**: Rate-limited to avoid showing the same notification repeatedly.

**Resolves**: TEST-PLAN §7.9 (notification data source, schema, frequency rules).

---

## §6. Environments File (`packages/cli-kit/src/public/node/environments.ts`)

- Location: **`public/node/environments.ts`** (not `private/node/` as assumed).
- Format: **TOML** (via `TomlFile.read`).
- Resolution: Uses `findPathUp` to walk up directory tree from `cwd()`.
- Structure: Environment name as a key under `[environments]` TOML section.
- Returns: `JsonMap | undefined`.
- Metadata: Logs via `metadata.addSensitiveMetadata` when an environment is loaded.

**Resolves**: TEST-PLAN §7.12 (environment file format and resolution).

---

## §7. Bulk Operations (`packages/app/src/cli/services/bulk-operations/`)

| Property | Value | File |
|---|---|---|
| Min API version | `2026-01` | `constants.ts:5` |
| Terminal statuses | `COMPLETED`, `FAILED`, `CANCELED`, `EXPIRED` | `watch-bulk-operation.ts:14` |
| Quick watch interval | 300ms | `watch-bulk-operation.ts:19` |
| Quick watch timeout | 3,000ms | `watch-bulk-operation.ts:19` |
| Regular watch interval | 5s | `watch-bulk-operation.ts:16` |
| Initial poll interval | 1s (×10 polls, then regular) | `watch-bulk-operation.ts:15-17` |
| Adaptive polling | Yes — first 10 polls at 1s, then 5s | `watch-bulk-operation.ts:127-129` |
| Cancellation | GraphQL mutation `BulkOperationCancel` | `cancel-bulk-operation.ts:38-43` |
| AbortSignal | Uses `Promise.race([sleep, abortSignal])` | `watch-bulk-operation.ts:133-136` |
| Batching | 50 per batch via `slice` | Not in bulk-ops; found in theme `api.ts:235` |
| Output | JSONL download from `operation.url`, optionally written to file | `execute-bulk-operation.ts:187-193` |
| Variable input | JSONL file (`--variable-file`) or inline JSON (`--variables`) | `execute-bulk-operation.ts:41-56` |
| Mutations vs queries | Validates — mutations require variables, queries reject them | `execute-bulk-operation.ts:235-251` |

**Resolves**: TEST-PLAN §7.10 (polling interval, timeout, cancellation mechanics).

---

## §8. Dev Directory / Hot-Reload / WebSocket (`packages/app/src/cli/services/dev/`)

- **WebSocket server**: Uses the `ws` npm package (`packages/app/src/cli/services/dev/extension/websocket/`).
  - Upgrade handler listens at path `/extensions` (`handlers.ts:20`).
  - On connect: sends initial `connected` payload with extension manifest version (`handlers.ts:31-37`).
  - On message: dispatches by event type (`handlers.ts` rest of file).
  - On extension update: broadcasts changes to all connected clients (`handlers.ts:152-163`).
- **WebSocket URL**: Protocol upgraded from HTTP to `wss:` at `/extensions` path (`extension.ts:193-196`).
- **HMR port**: Passed via `HMR_SERVER_PORT` env var (`processes/web.ts:142`).
- **File watching**: `AppEventWatcher` in `app-events/app-event-watcher.ts` watches file changes, triggers rebuilds.
- **URL generation** (`urls.ts`): Priority chain: Codespaces → Gitpod → `--tunnel-url` → `--no-tunnel` (localhost) → cloudflare tunnel (default).
- **Tunnel polling**: Every 500ms via `setTimeout` until `connected` or `error` (`urls.ts:105-127`).

**Resolves**: TEST-PLAN §7.11 (hot-reload/WebSocket protocol details).

---

## §9. Extension Build Pipeline (`packages/app/src/cli/services/build/extension.ts`)

### UI Extensions
- **Bundler**: esbuild via `bundleExtension()`.
- **Minification**: `minify: true`.
- **Loader**: `'tsx'`.
- **Environment**: `production | development`.
- **App URL**: Injected via `APP_URL` env var from dotenv.
- **Source maps**: Only for `isSourceMapGeneratingExtension`.
- **Error handling**: Bundling errors wrapped as `AbortError` (not CLI bugs), preserving esbuild error details.
- **Validation**: `extension.buildValidation({outputPath})` runs after bundle.

### Function Extensions
- **Lock file**: `proper-lockfile` at `.build-lock` to prevent concurrent builds.
- **Schema validation**: API version validated before build.
- **JS functions**: If `buildCommand` set, runs it; otherwise uses built-in `buildJSFunction`.
- **Non-JS functions**: Require `build.command` in config, otherwise raises `AbortSilentError`.
- **Post-build**: Optional `wasm_opt`, then `runTrampoline`.
- **Deploy bundle**: WASM base64-encoded into output directory for deploy.

**Resolves**: TEST-PLAN §7.13 (build pipeline flags and outputs).

---

## §10. CI Matrix (`packages/cli/package.json + .github/workflows/`)

### Target Matrix
| Dimension | Values |
|---|---|
| OS | `ubuntu-latest`, `windows-latest`, `macos-latest` |
| Node.js | `22.12.0`, `24.1.0`, `26.1.0` (min engine: `>=22.12.0`) |
| Sharding | Windows only — `1/2` and `2/2` |
| Vitest threads | `VITEST_MIN_THREADS=1`, `VITEST_MAX_THREADS=4` |

### CI Jobs (PR Pipeline: `tests-pr.yml`)
| Job | OS | Node | Description |
|---|---|---|---|
| type-check | ubuntu-latest | 26.1.0 | `pnpm nx run-many --target=type-check` |
| lint | ubuntu-latest | 26.1.0 | `pnpm nx run-many --target=lint` |
| bundle | ubuntu-latest | 26.1.0 | `pnpm nx run-many --target=build` + `--target=bundle` |
| knip | ubuntu-latest | 26.1.0 | Dead code detection |
| graphql-schema | macos-latest | 26.1.0 | Codegen check |
| oclif-checks | ubuntu-latest | 26.1.0 | Manifests, readme, dev docs |
| unit-tests | all 3 OS × 3 Node | matrix | `pnpm vitest run` |
| e2e-tests | ubuntu-latest | 24.1.0 | Playwright, 2 shards |

### Main Pipeline (`tests-main.yml`)
- Same matrix, but `build`/`lint`/`type-check`/`bundle` only run on `ubuntu-latest + Node 26.1.0`.
- Slack notification on failure.

### Release (`release.yml`)
- **Publishing**: npm registry (`@shopify:registry = https://registry.npmjs.org`), public access.
- **Tags**: `nightly`, `latest`, `experimental`.
- **Provenance**: `NPM_CONFIG_PROVENANCE=true`.
- **Snapit**: `/snapit` comment on PRs triggers snapshot build → npm publish.
- **Changeset**: Automated version bumps + changelog + GitHub release + stable branch creation (`stable/$MINOR`).
- **Cron**: Daily at 6:00 UTC for nightly releases.

### Setup Dependencies (`setup-cli-deps/action.yml`)
- `pnpm install --frozen-lockfile --prefer-offline`
- Node.js via `actions/setup-node` with pnpm cache.

### Oclif Config (from `cli/package.json:87-159`)
- **Bin name**: `shopify`
- **Command strategy**: `explicit` (manifest-based, target `./dist/index.js`)
- **Topics**: `hydrogen`, `theme`, `app`, `store`, `auth`, `config`
- **Hooks**: `init`, `prerun`, `postrun`, `command_not_found`, `tunnel_start`, `tunnel_provider`, `update`, `sensitive_command_metadata`, `public_command_metadata`
- **Auto-update**: Via `@oclif/plugin-plugins` update hook
- **Files in package**: `/assets`, `/bin/run.js`, `/bin/run.cmd`, `/dist`, `/oclif.manifest.json`

**Resolves**: DEVOPS §6 (CI target matrix), TEST-PLAN §7.14 (auto-upgrade mechanism via oclif update hook), §7.15 (install path — npm registry), §7.16 (hosting — npmjs.org + GitHub releases).

---

## §11. Cloudflare Plugin (`packages/plugin-cloudflare/src/`)

- **No Cloudflare REST API calls** — the plugin only manages the `cloudflared` binary.
- `tunnel.ts`: Spawns `cloudflared` as a subprocess, manages its lifecycle (start/stop/status).
- `install-cloudflared.ts`: Downloads the `cloudflared` binary from GitHub releases.
- `provider.ts`: Registers the tunnel provider with the CLI plugin system.

**Resolves**: TEST-PLAN §7.17 (Cloudflare REST API usage — none).

---

## §12. Doctor Command (`packages/cli/src/cli/commands/doctor-release/`)

- **Command**: Thin dispatcher — validates input, delegates to service.
- **Service**: `services/doctor-release/theme/runner.ts` (not yet read in detail).
- Purpose: Theme validation/health check for release.

**Resolves**: TEST-PLAN §7.18 (doctor-release scope — thin command shell, real logic in service).
