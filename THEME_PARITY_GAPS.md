# Theme Parity Gaps

Checklist for porting `packages/theme` from upstream Shopify CLI into the Rust theme crate.
Generated from deep analysis of upstream `packages/theme/src/cli` vs the Rust `crates/theme` + `cli-kit` boundary.

## Current Baseline

- Rust `theme` crate tests: `cargo test -p theme --lib` → **276 tests**.
- Rust CLI theme boundary tests: `cargo test -p cli-kit theme::` → **30 tests**.
- All 20 theme subcommands wired and dispatched in `cli-kit/src/commands/theme.rs`.
- Dev-server core + theme-ext environment live in `crates/theme`.

## Status Legend

- `[x]` = parity complete with tests.
- `[~]` = partial: core logic ported, specific behaviors/tests still missing.
- `[ ]` = not ported / feature missing.

## High Priority — Dev Server Behaviors

- [x] Dev server request logging (`log-request-line.ts`)
- [x] Theme-ID mismatch flow (`html.ts` `assertThemeId`)
- [x] Stale rendered-asset query rewrite
- [x] `getInMemoryTemplates` / SFR `replace_templates` POST for full-page rendering
- [x] Hot-reload path-agnostic middleware (SSE-on-page, section re-render, `hr-log`, local script, `LOCAL_HOT_RELOAD`)
- [x] Hot-reload payload fidelity (dual local/remote sync, `fileDetailsCache`, full unsynced bag)
- [x] Wildcard-bind host validation — enumerate NICs when bound to `0.0.0.0`/`::`
- [x] 4xx SFR → proxy fallback (`html.ts` `tryProxyRequest` / `is_known_rendering_request`)
- [x] Polaris-style error overlays (`get_error_page` + HR/inspector injection on render failures)

## Confirmed P0 CLI/Dev Regressions

- [x] `previewPath` uses `&` when editor URL already has `?hr=`
- [x] `standard_events_dev_bundle` defaults to `true`
- [x] `shpat_` warning on `theme dev`
- [x] Keypress debounce (100ms leading)

## High Priority — Theme Extension Environment

- [x] `theme-ext-fs` — whitelist mount, in-memory write/delete, 5ms unsynced clear, templates helpers
- [x] `theme-ext-server` — port 9293, host `127.0.0.1`, host validation, unconditional hot-reload SSE
- [x] Theme-ext HTML/proxy/SFR fallback (ignored paths 204; CDN/cart proxy; storefront HTML + `replace_extension_templates` + HR inject)
- [x] Theme-ext file-watcher triggers hot-reload Update events
- [x] Host theme find-or-create (Dawn zip + fallback catalog zip + wait-until-processed) + next-steps banner
- [x] Proxy differences for extensions in main theme `dev`
  - `/ext/cdn/` route + local extension asset serving
  - CDN rewrite of local ext assets → `/ext/cdn/...`
  - Bearer (`Authorization`) only when `DevServerKind::Theme`
  - Extension templates merged into SFR POST via `replace_extension_templates`

## High Priority — Dev Server Session

- [x] 30-min session refresh (`start_dev_session_refresh`)
- [x] `abortOnMissingRequiredFile` when `_shopify_essential` cannot be established
- [x] Essential-cookie HEAD sends Theme Access / shop auth headers
- [x] Session helper tests (headers, redirect location, missing-required-files message)

## Medium Priority — Command/Services Parity

- [x] `theme init` AI instruction flow; copilot at theme root
- [x] `theme preview` / check / language-server wrappers
- [x] `dev` success banner key hints (`t`/`p`/`e`/`g`) via `render_dev_links`
- [x] `package` includes `listings/**`, `release-notes.md`, `update_extension.json`
- [x] `push`/`dev` non-theme dir confirm via `ensure_directory_confirmed`
- [x] `push` `--json` errors as path-keyed map; publish live URL copy
- [x] list / info / duplicate JSON nesting tests
- [x] High-signal multi-env ThemeCommandRunner cases (missing path, missing store, env failure warning)
- [~] Remaining multi-env edge cases / progress UI / analytics (out of product scope)

## Test Coverage

### Ported / strengthened

- [x] plan_pull / plan_push ignore-only-mismatch-nodelete scenarios
- [x] theme-ext-fs / theme-ext-server unit tests (ignored 204, HTML SFR mock, local assets, HR on file change, host-theme manager)
- [x] hot-reload payload differential + previewPath/`&` + wildcard hosts
- [x] package zip membership for listings/release-notes/update_extension
- [x] push JSON path-keyed errors
- [x] 4xx known-rendering / error page / ext CDN rewrite / bearer-by-kind
- [x] poll dual-source abort + JSON checksum filters + reconcile→apply checksums
- [x] storefront-session helpers + `render_dev_links` + `resolve_port` taken
- [x] list/info/duplicate JSON shapes + multi-env path/store/failure cases

### Remaining (low priority / out of scope)

- [x] Storefront-session retry classifier (`should_retry_storefront_session` / 429/5xx / wrong-password)
- [ ] End-to-end `theme-environment.test.ts` bootstrap suite
- [ ] Progress-bar / analytics metadata parity
- [ ] Exact chalk/`renderConcurrent` streaming UI

## Known Regex Constraints (Rust)

- `regex-lite` has **no look-around**: `(?!extensions\/)`, `(?!internal\/)`, `(?=:\d|$)` require the full
  `regex` crate or rewritten logic. `checkouts` exclusion in `can_proxy_request` is already handled without look-around.
- Lazy `+?` inside the theme-id pattern fails on regex-lite; use greedy `[^}]*` (same capture result).
