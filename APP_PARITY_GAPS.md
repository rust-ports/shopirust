# App Parity Gaps

Checklist for porting `packages/app` from upstream Shopify CLI into the Rust `app` crate (+ cli-kit app commands / tunnel).
Updated after streams H–L (bulk Admin HTTP, Partners deploy, flow/schema/loader, function/logs/uninstall, console assets + TUI).

## Current Baseline

- Rust `app` crate tests: `cargo test -p app --lib` → **773 tests** (Waves A–E production paths + high-value cases; not a 1:1 Vitest clone).
- Upstream Vitest surface for `packages/app` is still ~**2082**. This crate is **not** at that count; `[x]` below means behavior + tests for that item, not a 1:1 Vitest clone.
- Spec registry: **26/26** identifiers with `deploy_config` / transforms / validation hooks.
- Commands wired: init, generate, import-*, info, config, build, deploy, release, versions, bulk, function, execute, env, webhook, logs, **dev** (+ clean).
- cli-kit app clap tests: **21** (`cargo test -p cli-kit --lib commands::app`).

## Status Legend

- `[x]` = parity complete with tests (or production-usable).
- `[~]` = partial: core logic ported, specific behaviors/tests still thinner than Vitest.
- `[ ]` = not ported / intentionally deferred.

## P0 — Extension specifications

- [x] Spec engine (`deploy_config`, transforms, validate, `patch_with_app_dev_urls`, `UidStrategy`)
- [x] Shared schemas + first-class field stripping
- [x] Path-map transforms (branding, app_access, app_home, point_of_sale)
- [x] Custom transforms (app_proxy, webhooks, webhook_subscription, privacy_compliance, events)
- [x] Module deploy configs: function, theme, ui_extension, checkout_*, pos_ui, product_subscription, web_pixel, tax, editor_collection, flow_*, payments, contract modules
- [x] Locales loader for deploy payloads
- [x] Exact Zod refine rules for payments targets (offsite/credit-card/custom/redeemable/card-present: 3DS, oversell, installments, fingerprint, field cap, buyer_label locale)
- [x] Per-spec modules under `models/extensions/specifications/` (`ui_extension`, `payments`, config transforms, function/flow/admin_link/editor_collection) with deploy/validate tests
- [x] UI-extension `shopify.d.ts` string emission (`models/extensions/type_generation.rs`)

## P1 — Loader / context / deploy / build

- [x] `id_matching` automatic matchmaking (UID/UUID/name+type)
- [x] Deploy manifest includes per-extension `config` from `deploy_config`
- [x] Config writers (`write` / `patch` / `add_uid_to_extension_toml` / hidden-config by client_id)
- [x] Real `config link` / `config pull` (auth, org/app select, `fetch_app_remote_configuration`, merge, write, preference)
- [x] Shared `AppLinkedArgs` (`--path` / `--config` / `--client-id` / `--reset`, exclusive with `--config`)
- [x] App prompts module (`crates/app/src/prompts`) with injected answers for tests
- [x] AM `dev_stores_for_org` + `store_by_domain` (Business Platform); `OrganizationApp` URLs from app_home/app_access
- [x] `store_context` (flag → cached `dev_store_url` → prompt) wired into `dev` / `dev clean` / `logs` / `execute`
- [x] `linked_app_context` `force_relink` (`--reset`) and auto-link when unlinked
- [x] Deploy confirm table + `--allow-deletes` / `--allow-updates` (TTY + non-TTY)
- [x] Manual ID matching prompts; AM atomic deploy does **not** invent `pending:{handle}` UUIDs
- [x] `import-extensions` remote `app_extension_registrations` fetch (file still optional)
- [x] Include-assets step matrix (static / pattern / configKey / `[]` flatten / `assert_path_within_app_dir` / manifest)
- [x] Init catalog (`visibleTemplates` + flavor branches + optional `--name`/`--template`/`--client-id`) and generate catalog (`template_specifications` + flavor subdir)
- [x] Loader URL / webhook URI+dup / missing entry source / `include_config_on_deploy` filter (targeted tests; not the 15 fat TOML novels)
- [~] Loader Vitest volume (~30 tests vs ~184 upstream)
- [x] Partners-only deploy path: `create_extension`, signed upload URL, `appModules`/`skipPublish`, import-on-deploy, `include_config_on_deploy`

## P2 — env / webhook

- [x] `app env pull|show`
- [x] `app webhook trigger`: `WebhooksClient` (`publicApiVersions` / `availableTopics` / `cliTesting`); flags optional (prompts fill them); localhost POSTs the sample; HTTP / Pub/Sub / EventBridge enqueue via the API
- [x] `send_uninstall_webhook_to_app_server` live sample + HMAC; `app dev` threads `WebhookSampleClient` (live first, synthetic fallback; 3s / 5s / 3 retries)
- [x] Ported webhook Vitest: trigger, trigger-options, trigger-flags, request-sample / topics / api-versions, trigger-local-webhook, send-app-uninstalled, prompt/webhook

## P3 — logs

- [x] `app logs` + `app logs sources`
- [x] `AppLogsPoller` reusable by T7
- [x] `camelcase_keys` (shallow + `deep`) + `to_formatted_app_log_json` / text render tests
- [x] Poll transport/429/5xx retry (5s) + 401 resubscribe fail ×5 session-expired; JSON error lines; default write `.shopify/logs`
- [~] Full Ink/UI log component parity (`Logs.tsx`, `usePollAppLogs`, `useSelfAdjustingInterval` not ported)

## P4 — Niche builders

- [x] `services/flow` serialize/validate/config builders wired into `deploy_flow_action` / `deploy_flow_trigger`
- [x] AM `json_schema` on specs + `jsonschema` crate draft-07 (`$ref`, `allOf`/`anyOf`, nested `required`, `enum`, `pattern`)
- [x] `services/payments` per-target helpers
- [x] `admin_link` / `marketing_activity` / `subscription_link` import helpers
- [x] Hooked into `import_extensions`

## P5 — App dev (T7)

- [x] `app dev` + `app dev clean` CLI
- [x] Tunnel mode + Cloudflare/`cloudflared` client (+ FakeTunnel)
- [x] Process setup: web, previewable, draftable, theme-ext, GraphiQL CDN explorer, APP_UNINSTALLED HMAC POST, app logs polling, app watcher, dev session, reverse proxy
- [x] `--no-update` / skip-deps / `--notify`; mkcert localhost TLS; `https://localhost:{port}` application URLs
- [x] Vendored ui-extensions-dev-console assets (`rust-embed`; `GET /extensions/dev-console` + `/assets/*`; `make console-assets`)
- [x] Dev session subscribes to `AppEventWatcher` and calls `devSessionUpdate`; `DevSessionStatusManager`
- [x] TTY concurrent prefixed logs + status table + shortcuts `p`/`g`/`q` (cli-kit widget UX, not Ink)
- [~] Pixel-identical Ink DevSessionUI
- [x] Theme-app-extension host via `crates/theme` (`theme_ext`)

## P6 — Tests / docs

- [x] This checklist
- [x] README updated for newly ready commands (webhook trigger now documents optional flags + live EventBridge/PubSub)
- [x] cli-kit clap boundary tests for linked flags, init, generate, execute, webhook, bulk, env, function, release, versions, logs, import
- [x] `assert_cmd` E2E smoke (help/version/validate/invalid TOML/search/theme/did-you-mean) in `crates/cli-kit/tests/e2e.rs`
- [ ] Playwright-class live Shopify E2E (out of scope)

## Stream G mop-up (honest remainder vs ~2082)

Ported in this pass (on top of A–F):

- Bulk status formatting / cancel follow-up / user-error rendering (pure functions + tests; Admin HTTP execute/cancel/watch still thinner than Vitest)
- App-logs camelCase + JSON/text render
- `utilities/json_schema` (required-property merge; not full AJV)
- `utilities/execute_helpers` (`resolve_graphql_query` / `validate_single_operation`)
- Locales loader extra cases (empty dir, non-JSON, invalid JSON, base64)
- Prompt coverage expanded (init/generate/import/org/store/webhook/injected prompter)
- Function replay: skip runs without input, empty-dir error, filename identifier

Still thinner than Vitest (do **not** claim 2082):

- [~] Loader + models/app project tests (targeted vs ~184)
- [x] Bulk Admin HTTP execute/watch/download/staged-upload + list `/bulkOperation` parse
- [x] Flow serialize/validate in deploy + jsonschema draft-07
- [x] Function replay `--watch` (wasm mtime), multi-function/export select, `wasm32-unknown-unknown` fallback, download lock; replay does not require `linked_app_context`
- [x] App-logs poll error/retry matrix (429 / 5xx / transport / 401×5)
- [ ] Ink/React UI tests (DevSessionUI, function Replay UI, logs UI)

## Remaining (follow-ups)

- [ ] Playwright-class live Shopify E2E
- [x] Store topic + CLI meta live in `crates/store` + `cli-kit` (not this crate)
- [~] Pixel-identical Ink DevSessionUI / Replay.tsx
- [~] Vitest volume still below upstream 2082; high-value loader/deploy/context/migrate/validation cases are ported.
