# CLI-Kit Phase Plan & Checklist

**Current**: ~8,200 LOC / 52 files / ~30% of upstream cli-kit
**Target**: ~22,000 LOC / 100+ files / 100% of upstream cli-kit

---

## Phase 2 — API Client Methods (+3,000 LOC)

### 2.1 `api/app_management.rs` (58 → ~400 LOC)
- [ ] Struct `AppManagementClient` with `GraphqlClient` + token
- [ ] `new(token)` — base URL from `constants::app_management_fqdn()`
- [ ] `organizations()` — list orgs the user has access to
- [ ] `org_from_id()` — single org by ID
- [ ] `create_app()` — create a new app
- [ ] `update_urls()` — update app URLs
- [ ] `app_from_id()` — get app by ID
- [ ] `app_from_id_basic()` — basic app info
- [ ] `app_from_name()` — find app by name
- [ ] `app_extension_registrations()` — all extension registrations
- [ ] `specifications()` — extension specification types
- [ ] `template_specifications()` — template specifications
- [ ] `deploy()` — deploy app version
- [ ] `release()` — release app version
- [ ] `dev_session_create()` — create dev session
- [ ] `dev_session_update()` — update dev session
- [ ] `dev_session_delete()` — delete dev session
- [ ] `generate_signed_upload_url()` — get upload URL
- [ ] `active_app_version()` — current version
- [ ] `app_versions()` — version history
- [ ] `app_versions_diff()` — diff two versions
- [ ] Wiremock tests for 3 key methods

### 2.2 `api/business_platform.rs` (49 → ~300 LOC)
- [ ] Struct `BusinessPlatformClient`
- [ ] `new()` — two base URLs (Destinations + Organizations)
- [ ] `destinations_query()` — destinations API query
- [ ] `organizations_query()` — organizations API query
- [ ] `org_by_hashed_email()` — find org by email hash
- [ ] `user_email()` — get user email
- [ ] Wiremock tests

### 2.3 `api/functions.rs` (41 → ~200 LOC)
- [ ] Struct `FunctionsClient`
- [ ] `api_schema_definition()` — schema for a given API
- [ ] `target_schema_definition()` — schema for function target
- [ ] `function_active_version()` — active function version
- [ ] Wiremock tests

### 2.4 `api/webhooks.rs` (38 → ~200 LOC)
- [ ] Struct `WebhooksClient`
- [ ] `api_versions()` — available API versions
- [ ] `topics()` — available webhook topics
- [ ] `send_sample_webhook()` — send test webhook
- [ ] Wiremock tests

### 2.5 `api/app_dev.rs` (24 → ~200 LOC)
- [ ] Struct `AppDevClient`
- [ ] `dev_session_create()`
- [ ] `dev_session_update()`
- [ ] `dev_session_delete()`
- [ ] Wiremock tests

### 2.6 `api/partners.rs` (988 → ~1,500 LOC)
- [ ] Audit against upstream `partners.ts` — add missing methods
- [ ] Verify all upstream query/mutation wrappers exist
- [ ] Full test coverage for all methods

### 2.7 `api/admin.rs` (386 → ~800 LOC)
- [ ] `list_themes()` — all themes for a store
- [ ] `get_theme()` — single theme by ID
- [ ] `create_theme()` — create new theme
- [ ] `update_theme()` — update theme metadata
- [ ] `delete_theme()` — delete theme
- [ ] `duplicate_theme()` — duplicate existing theme
- [ ] `publish_theme()` — publish/unpublish theme
- [ ] `get_theme_file_bodies()` — fetch file contents
- [ ] `get_theme_file_checksums()` — fetch file checksums
- [ ] `upsert_theme_files()` — create/update theme files
- [ ] `delete_theme_files()` — delete theme files
- [ ] `public_api_versions()` — discover available API versions (cached)
- [ ] `metafield_definitions_by_owner_type()`
- [ ] `online_store_password_protection()`
- [ ] Wiremock tests for every method

---

## Phase 3 — UI System (+3,500 LOC)

### 3.1 `output/inflector.rs` (new, ~100 LOC)
- [ ] Format styles: bold, italic, subdued, error, info, warning
- [ ] Token types: Command, Link, FilePath, UserInput
- [ ] Match upstream `content-tokens.ts` (131 LOC) + `output.ts` (447 LOC)

### 3.2 `output/banner.rs` (173 → ~400 LOC)
- [ ] Info/warning/success/error banner types with colors
- [ ] Headings with accent color
- [ ] Next-steps rendering
- [ ] Fatal error with stack trace
- [ ] Match upstream `Banner.tsx` (112 LOC) + `FatalError.tsx` (100 LOC)

### 3.3 `output/prompt.rs` (131 → ~500 LOC)
- [ ] `SelectPrompt` — scrollable list selection
- [ ] `AutocompletePrompt` — searchable/filterable selection
- [ ] `TextPrompt` — free-text input
- [ ] `DangerousConfirmationPrompt` — type-to-confirm
- [ ] `SelectInput` component — keyboard-navigable list
- [ ] Match upstream `SelectPrompt.tsx` (70 LOC), `AutocompletePrompt.tsx` (197 LOC), `TextPrompt.tsx` (149 LOC), `DangerousConfirmationPrompt.tsx` (166 LOC), `SelectInput.tsx` (320 LOC)

### 3.4 `output/tasks.rs` (new, ~300 LOC)
- [ ] Task runner with states: pending → running → done/failed/skipped
- [ ] Sub-task nesting
- [ ] Elapsed time display
- [ ] Spinner animation
- [ ] Match upstream `Tasks.tsx` (117 LOC) + `SingleTask.tsx` (61 LOC)

### 3.5 `output/table.rs` (new, ~200 LOC)
- [ ] Table builder: headers, rows, column config
- [ ] Column alignment, borders, padding
- [ ] Match upstream `Table.tsx` (58 LOC), `Row.tsx` (50 LOC), `Column.ts` (5 LOC)

### 3.6 `output/alert.rs` (new, ~150 LOC)
- [ ] Alert types: info, warning, error, success
- [ ] Styled message boxes with icons
- [ ] Match upstream `Alert.tsx` (74 LOC)

### 3.7 `output/list.rs` (new, ~150 LOC)
- [ ] Ordered (numbered) lists
- [ ] Unordered (bullet) lists
- [ ] Match upstream `List.tsx` (93 LOC)

### 3.8 `output/link.rs` (new, ~80 LOC)
- [ ] Hyperlink rendering with terminal detection
- [ ] Match upstream `Link.tsx` (42 LOC)

### 3.9 `output/text_input.rs` (new, ~200 LOC)
- [ ] Inline text input with cursor, placeholder, masking
- [ ] Match upstream `TextInput.tsx` (125 LOC)

### 3.10 `output/loading_bar.rs` (new, ~100 LOC)
- [ ] Animated progress bar with percentage
- [ ] Match upstream `LoadingBar.tsx` (41 LOC)

### 3.11 `output/concurrent_output.rs` (new, ~300 LOC)
- [ ] Interleaved output from concurrent processes with prefixes
- [ ] Match upstream `ConcurrentOutput.tsx` (233 LOC)

### 3.12 `output/mod.rs` (193 → ~300 LOC)
- [ ] Re-export all new modules
- [ ] `format_message` with inflector
- [ ] Verbosity levels

---

## Phase 4 — Session OAuth Completion (+1,500 LOC)

### 4.1 `session/identity.rs` (16 → ~100 LOC)
- [ ] JWT extraction: sub (user ID), exp, scopes
- [ ] ID token validation with JWKS
- [ ] Match upstream `identity.ts` (69 LOC)

### 4.2 `session/device_authorization.rs` (93 → ~200 LOC)
- [ ] Complete polling loop: `/oauth/device/code` → poll `/oauth/device/token`
- [ ] Timeout handling
- [ ] Cancellation support
- [ ] Resume from stored `PendingDeviceAuth`
- [ ] Match upstream `device-authorization.ts` (190 LOC)

### 4.3 `session/exchange.rs` (294 → ~500 LOC)
- [ ] `exchange_custom_partner_token()` — env → Partners API token
- [ ] `exchange_app_automation_token_for_app_management()`
- [ ] `exchange_app_automation_token_for_business_platform()`
- [ ] `exchange_identity_token_for_api_token()` — identity → API surface
- [ ] Match upstream `exchange.ts` (287 LOC)

### 4.4 `session/validate.rs` (240 → ~350 LOC)
- [ ] Scope comparison: requested ⊆ stored
- [ ] `needs_refresh()` with `SESSION_EXPIRATION_MARGIN_MINUTES`
- [ ] Match upstream `validate.ts` (80 LOC) + `scopes.ts` (87 LOC)

### 4.5 `session/store.rs` (135 → ~250 LOC)
- [ ] Multi-session support (multiple stores, orgs)
- [ ] Keychain integration (Linux secret-service)
- [ ] Match upstream `store.ts` (107 LOC)

### 4.6 `session/scopes.rs` (71 → ~150 LOC)
- [ ] `scope_string()`, `scope_set()`, `scope_intersection()`, `scope_difference()`
- [ ] Match upstream `scopes.ts` (87 LOC)

### 4.7 `session/schema.rs` (31 → ~100 LOC)
- [ ] Full session type: exchange, identity, expires, scopes
- [ ] Match upstream `schema.ts` (78 LOC)

### 4.8 `session/mod.rs` (411 → ~800 LOC)
- [ ] `ensure_authenticated_partners()`
- [ ] `ensure_authenticated_admin()`
- [ ] `ensure_authenticated_themes()`
- [ ] `ensure_authenticated_app_management_and_business_platform()`
- [ ] `ensure_authenticated_storefront()`
- [ ] `ensure_authenticated_business_platform()`
- [ ] `ensure_authenticated_admin_as_app()`
- [ ] Each: env check → store → OAuth fallback
- [ ] Match upstream `session.ts` (357 LOC) + `session-prompt.ts` (115 LOC)

---

## Phase 5 — Analytics + Error Reporting (+1,000 LOC)

### 5.1 `util/analytics.rs` (89 → ~500 LOC)
- [ ] Monorail event schema: `cli/command_exec/1.0`, `cli/command_abort/1.0`
- [ ] Event types: CommandExec, CommandAbort, FatalError
- [ ] Batching: 30s interval or 100 events
- [ ] Retry on flush failure (exponential backoff)
- [ ] Anonymous vs authenticated (hashable identity)
- [ ] Metadata: version, OS, platform, project, command, duration
- [ ] Match upstream `monorail.ts` (265 LOC) + `private/node/analytics/` (~740 LOC)

### 5.2 `util/error_handler.rs` (new, ~300 LOC)
- [ ] Catch panics, format stack trace
- [ ] Environment context collection
- [ ] Rate-limited (max 1/min)
- [ ] Match upstream `error-handler.ts` (318 LOC) + `error-categorizer.ts` (114 LOC)

### 5.3 `constants.rs` — new endpoints
- [ ] `MONORAIL_ENDPOINT`
- [ ] `ERROR_ANALYTICS_ENDPOINT`

---

## Phase 6 — Systems & Utilities (+2,500 LOC)

### 6.1 `util/system.rs` (156 → ~500 LOC)
- [ ] `execute_subprocess()` — spawn + capture output, timeout
- [ ] `open_url()` — `xdg-open` / system open
- [ ] `tree_kill()` — kill process tree
- [ ] `check_port()` — TCP port availability
- [ ] `is_global_installation()` — detect global install
- [ ] Match upstream `system.ts` (411 LOC) + `tree-kill.ts` (230 LOC) + `tcp.ts` (111 LOC)

### 6.2 `util/fs.rs` (129 → ~400 LOC)
- [ ] `archive_zip()` / `extract_tar()` — port `archiver.ts` (203 LOC)
- [ ] `find_up()` — walk up directories
- [ ] `is_hidden()` / `hidden_folder()`
- [ ] `glob()` — file pattern matching
- [ ] Extended file ops (move, copy with permissions)
- [ ] Match upstream `fs.ts` (727 LOC)

### 6.3 `util/github.rs` (108 → ~250 LOC)
- [ ] `download_github_release()` — fetch release artifacts
- [ ] `get_latest_github_release()` — check releases API
- [ ] Match upstream `github.ts` (174 LOC)

### 6.4 `util/dot_env.rs` (new, ~200 LOC)
- [ ] `.env` file parsing, variable substitution
- [ ] Local env detection (Spin, Codespaces, Gitpod, CloudShell)
- [ ] Match upstream `dot-env.ts` (143 LOC) + `context/local.ts` (328 LOC)

### 6.5 `util/path.rs` (new, ~250 LOC)
- [ ] Cross-platform path resolution
- [ ] Temp directory management
- [ ] Match upstream `path.ts` (236 LOC) + `temp-dir.ts` (8 LOC)

### 6.6 `util/result.rs` (new, ~150 LOC)
- [ ] Result-like monad for nullable JS patterns
- [ ] Match upstream `result.ts` (145 LOC)

### 6.7 `util/package_manager.rs` (84 → ~300 LOC)
- [ ] `detect_package_manager()` — lockfile + global detection
- [ ] `install_packages()` — run install
- [ ] `add_packages()` — add deps
- [ ] `run_script()` — execute scripts
- [ ] `get_package_version()` — read version
- [ ] Workspace support (pnpm, npm workspaces)
- [ ] Match upstream `node-package-manager.ts` (798 LOC) — subset needed

### 6.8 `util/context.rs` (23 → ~100 LOC)
- [ ] Deprecations store
- [ ] Service context
- [ ] Context utilities
- [ ] Match upstream `private/node/context/` (~150 LOC total)

### 6.9 New files
- [ ] `util/framework.rs` — framework detection (~200 LOC, upstream `framework.ts` 199 LOC)
- [ ] `util/import_extractor.rs` — import scanning (~270 LOC, upstream `import-extractor.ts` 270 LOC)
- [ ] `util/request_ids.rs` — x-request-id tracking (~50 LOC, upstream `request-ids.ts` 43 LOC)

---

## Phase 7 — CLI Commands (+1,000 LOC)

### 7.1 `commands/cache/clear.rs`
- [ ] Remove all cached files from `cache_path()`

### 7.2 `commands/config/autoupgrade/{on,off,status}.rs`
- [ ] Toggle auto-upgrade in config store
- [ ] Show current status

### 7.3 `commands/upgrade.rs`
- [ ] Check latest version on GitHub
- [ ] Download + install (cargo install or binary)
- [ ] Match upstream `upgrade.ts` (299 LOC)

### 7.4 `commands/search.rs`
- [ ] Search docs.shopify.com
- [ ] Match upstream `cli/services/commands/search.ts`

### 7.5 `commands/notifications/{list,generate}.rs`
- [ ] List/generate notifications
- [ ] Match upstream `notifications-system.ts` (323 LOC)

### 7.6 `commands/kitchen_sink/{static,async,prompts}.rs`
- [ ] UI component demos
- [ ] Match upstream `services/kitchen-sink/`

### 7.7 `commands/debug/command_flags.rs`
- [ ] Show parsed flags for debugging

---

## Phase 8 — `cli-api` Crate (+1,000 LOC)

### 8.1 New crate `crates/cli-api/`
- [ ] `Cargo.toml` with `cli-kit`, `async-trait`, `serde` deps

### 8.2 `crates/cli-api/src/traits.rs`
- [ ] `DeveloperPlatformClient` trait with ~50 methods
- [ ] `organizations()`, `org_and_apps()`, `org_from_id()`
- [ ] `app_from_id()`, `app_from_id_basic()`, `app_from_name()`
- [ ] `app_extension_registrations()`, `specifications()`, `template_specifications()`
- [ ] `create_app()`, `update_urls()`, `deploy()`, `release()`
- [ ] `dev_session_create/update/delete()`, `generate_signed_upload_url()`
- [ ] `active_app_version()`, `app_versions()`, `app_versions_diff()`
- [ ] `send_sample_webhook()`, `api_versions()`, `topics()`
- [ ] `subscribe_to_app_logs()`, `app_logs()`
- [ ] `target_schema_definition()`, `api_schema_definition()`
- [ ] `create_extension()` etc.

### 8.3 `crates/cli-api/src/select.rs`
- [ ] `select_developer_platform_client()` — runtime resolution

### 8.4 `crates/cli-api/src/partners.rs`
- [ ] `PartnersClient` implements `DeveloperPlatformClient`

### 8.5 `crates/cli-api/src/app_management.rs`
- [ ] `AppManagementClient` implements `DeveloperPlatformClient`

---

## Phase 9 — Theme Crate (+3,500 LOC)

New crate `crates/theme/`. Details in PORT.md §5.3.
Commands: `push`, `pull`, `dev`, `delete`, `list`, `info`, `open`, `share`, `check`.

---

## Phase 10 — App Crate (+5,000 LOC)

New crate `crates/app/`. Details in PORT.md §5.2.
Commands: `dev`, `deploy`, `build`, `init`, `generate`, `env`, `config`, `function`, `logs`, `versions`, `webhook`, `release`, `bulk`.

---

## Phase 11 — Remaining Domain Crates (+3,000 LOC)

- [ ] `store` crate — `store create` command
- [ ] `organizations` crate — org selection
- [ ] `plugin-cloudflare` crate — tunnel management
- [ ] `plugin-did-you-mean` crate — command autocorrection
- [ ] `cli` crate — main binary wiring (update for new commands)
- [ ] `create-app` crate — standalone binary

---

## Phase 12 — Polish & Release (+1,500 LOC)

- [ ] `cargo doc` — fix all doc warnings
- [ ] Full integration test suite — every command
- [ ] Cross-compilation test (musl, Windows, macOS)
- [ ] CI pipeline hardening
- [ ] Performance profiling + optimization
- [ ] README + contribution docs
- [ ] Version alignment with upstream
