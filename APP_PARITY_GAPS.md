# App Parity Gaps

Checklist for porting `packages/app` from upstream Shopify CLI into the Rust `app` crate (+ cli-kit app commands / tunnel).
Generated after Full Parity + All Spec Types completion pass (P0–P6).

## Current Baseline

- Rust `app` crate tests: `cargo test -p app --lib` → **262+ tests**.
- Spec registry: **26/26** identifiers with `deploy_config` / transforms / validation hooks.
- Commands wired: init, generate, import-*, info, config, build, deploy, release, versions, bulk, function, execute, env, webhook, logs, **dev** (+ clean).

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
- [~] Exact Zod refine rules for every payments target / flow field shape (core paths covered)
- [~] Full Vitest ports of `ui_extension.test.ts` / `payments_app_extension.test.ts` volume

## P1 — Loader / context / deploy / build

- [x] `id_matching` automatic matchmaking (UID/UUID/name+type)
- [x] Deploy manifest includes per-extension `config` from `deploy_config`
- [x] Config writers (`write` / `patch` / `add_uid_to_extension_toml`)
- [x] `import-extensions` remote `app_extension_registrations` fetch (file still optional)
- [~] Interactive manual ID matching prompts
- [~] Full include-assets / tax-stub / theme-extension-config build step matrix
- [~] Partners-only deploy path depth

## P2 — env / webhook

- [x] `app env pull|show`
- [x] `app webhook trigger` (sample + HTTP HMAC delivery)
- [~] EventBridge / PubSub live delivery beyond address validation

## P3 — logs

- [x] `app logs` + `app logs sources`
- [x] `AppLogsPoller` reusable by T7
- [~] Full Ink/UI log component parity

## P4 — Niche builders

- [x] `services/flow` serialize/validate/config builders
- [x] `services/payments` per-target helpers
- [x] `admin_link` / `marketing_activity` / `subscription_link` import helpers
- [x] Hooked into `import_extensions`

## P5 — App dev (T7)

- [x] `app dev` + `app dev clean` CLI
- [x] Tunnel mode + Cloudflare/`cloudflared` client (+ FakeTunnel)
- [x] Process setup: web, previewable, draftable, theme-ext, graphiql stub, uninstall webhook, app logs polling, app watcher, dev session
- [x] Dev console HTML with WebSocket client (not full Polaris console assets)
- [~] Full ui-extensions-dev-console static asset bundle
- [~] Pixel-perfect concurrent TUI / Ink DevSessionUI
- [~] Live theme-app-extension environment depth vs `crates/theme`

## P6 — Tests / docs

- [x] This checklist
- [x] README updated for newly ready commands
- [~] cli-kit clap boundary tests for every new subcommand
- [ ] Playwright-class E2E (out of crate scope; track separately)

## Remaining (follow-ups)

- [ ] Bundle and serve full `ui-extensions-dev-console` assets
- [ ] Raise specification unit tests toward upstream Vitest density (~2000 app cases)
- [ ] Store topic / CLI meta (upgrade, notifications) — **out of app crate scope**
