# CLI-Kit Phase Plan & Checklist

**Current**: ~19,574 LOC / 104 files / ~40% of upstream cli-kit
**Target**: ~22,000 LOC / 100+ files / 100% of upstream cli-kit

---

## Phase 2 — API Client Methods (+3,000 LOC)

### 2.1 `api/app_management.rs` (282 → ~700 LOC)
- [x] Struct `AppManagementClient` with `GraphqlClient` + token
- [x] `new(token)` — base URL from `constants::app_management_fqdn()`
- [x] Generic `request()` method + rate limiter + deprecation handling
- [x] `with_graphql()` — inject pre-configured `GraphqlClient` (test mode)
- [x] `organizations()` — list orgs user has access to
- [x] `org_from_id()` — single org by ID
- [x] `create_app()` — create a new app
- [x] `update_urls()` — update app URLs
- [x] `app_from_id()` — get app by API key
- [x] `app_from_id_basic()` — basic app info
- [x] `app_from_name()` — find app by name
- [x] `app_extension_registrations()` — extensions from active release
- [x] `specifications()` — extension specification types
- [x] `template_specifications()` — template specifications
- [x] `deploy()` — deploy app version
- [x] `release()` — release app version
- [x] `generate_signed_upload_url()` — get upload URL
- [x] `active_app_version()` — current version
- [x] `app_versions()` — version history
- [x] `app_version_by_id()` — single version by ID
- [x] `app_versions_diff()` — diff two versions
- [x] Wiremock tests for 3 key methods (organizations, specifications, app_versions)
- [x] `app_version_by_id()` helper query
- [ ] `dev_session_create()` — in `app_dev.rs`
- [ ] `dev_session_update()` — in `app_dev.rs`
- [ ] `dev_session_delete()` — in `app_dev.rs`

### 2.2 `api/business_platform.rs` (128 → ~300 LOC)
- [x] Struct `BusinessPlatformClient`
- [x] `new()` — two base URLs (Destinations + Organizations)
- [x] Generic `request()` + `organizations_request()`
- [x] `destinations_query()` — destinations API query
- [x] `organizations_query()` — organizations API query
- [x] `org_by_hashed_email()` — find org by email hash
- [x] `user_email()` — get user email
- [ ] Wiremock tests

### 2.3 `api/functions.rs` (121 → ~200 LOC)
- [x] Struct `FunctionsClient`
- [x] `new()` + generic `request()` + rate limiter
- [x] `api_schema_definition()` — schema for a given API
- [x] `target_schema_definition()` — schema for function target
- [x] `function_active_version()` — active function version
- [ ] Wiremock tests

### 2.4 `api/webhooks.rs` (113 → ~200 LOC)
- [x] Struct `WebhooksClient`
- [x] `new()` + generic `request()` + rate limiter
- [x] `api_versions()` — available API versions
- [x] `topics()` — available webhook topics
- [x] `send_sample_webhook()` — send test webhook
- [ ] Wiremock tests

### 2.5 `api/app_dev.rs` (127 → ~200 LOC)
- [x] Struct `AppDevClient` with `x-forwarded-host` injection
- [x] `new()` + generic `request()` + rate limiter
- [x] `dev_session_create()`
- [x] `dev_session_update()`
- [x] `dev_session_delete()`
- [ ] Wiremock tests

### 2.6 `api/partners.rs` (1,045 → ~1,500 LOC)
- [x] All 11 domain methods implemented (organizations, apps, extensions, deploy, stores, account)
- [x] 5 GraphQL query constants + response types
- [x] 12 wiremock tests

### 2.7 `api/admin.rs` (410 → ~800 LOC)
- [x] Struct `AdminClient` + `AdminError` + query/rest_request infrastructure
- [x] `fetch_latest_api_version()` with caching per store
- [x] `fetch_api_versions()` with 403/401/404 handling
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

## Phase 3 — UI System (ratatui rewrite, ~5,650 LOC) ✅

**Architecture**: ratatui-based Ink-equivalent engine. Every component renders to either `RenderMode::Ansi` (static colored string for non-TTY/tests) or `RenderMode::Tui` (ratatui Frame for interactive TTY). Engine runs event loop via crossterm + tokio. No virtual DOM — canvas-style redraw. Static components (Banner, Alert, List, Link, Table, FatalError) stay as `colored` ANSI functions; only interactive (prompts, text input) and streaming (concurrent output, tasks) use ratatui.

### 3A — Foundation (~900 LOC, 15 files)

- [x] **3A.1** Add deps: ratatui, crossterm, tokio to Cargo.toml
- [x] **3A.2** `output/engine/mod.rs` — `RenderMode`, `RenderContext`, `RenderFragment` (Ansi/Span split), `Component` trait
- [x] **3A.3** `output/engine/event_loop.rs` — crossterm event stream → `Event` enum, dispatch to component, re-render cycle
- [x] **3A.4** `output/engine/lifecycle.rs` — `run_prompt()` (interactive loop), `run_streaming()` (channel-based loop), `render_static()` (one-shot String)
- [x] **3A.5** `output/engine/contexts.rs` — `CompletionContext`, `LinksContext` passed through `RenderContext`
- [x] **3A.6** `output/engine/layout.rs` — `calculate_layout()` → `Layout { two_thirds, one_third, full_width }`
- [x] **3A.7** `output/tokens/mod.rs` — `ContentToken<T>` trait, unify old token.rs + inflector.rs into one system
- [x] **3A.8** `output/tokens/token_item.rs` — `Token` UN, `TokenItem<T>`, `InlineToken`, `BoldToken`, `LinkToken`, `ListToken` types matching upstream discriminated unions
- [x] **3A.9** `output/tokens/tokenized_text.rs` — `TokenizedText` rendering engine: block/inline splitting, markdown link detection (`[label](url)` + `<url>`), dispatch to sub-components
- [x] **3A.10** `output/tokens/lines_diff.rs` — `LinesDiffContentToken` rendering `Change[]` as green `+`/magenta `-`
- [x] **3A.11** `output/colors.rs` — Color function wrappers (cyan, gray, magentaBright, etc.)
- [x] **3A.12** `output/figures.rs` — Unicode symbols: `✔`, `✖`, `•`, `◆`, `‖`, `─`, `△`, `▽`, `■`, `▔`, `│`, `║`, `◉`, `→`
- [x] **3A.13** `output/utilities.rs` — `message_with_punctuation()` helper
- [x] **3A.14** Remove old `token.rs`, `inflector.rs` — replaced by `tokens/` module
- [x] **3A.15** All 3A tests pass, clippy clean

### 3B — Static Components (~700 LOC, 12 files)

Small rendering-only components. Each produces `RenderFragment` (ANSI string or ratatui Span). Parent components compose them.

- [x] **3B.1** `output/components/command.rs` — `` `{command}` `` in magentaBright
- [x] **3B.2** `output/components/user_input.rs` — `{text}` in cyan
- [x] **3B.3** `output/components/subdued.rs` — `{text}` in dim
- [x] **3B.4** `output/components/file_path.rs` — `{path}` in italic
- [x] **3B.5** `output/components/link.rs` — hyperlink `\x1b]8;;` vs `label (url)` vs footnote `[N]`, `LinksContext` integration
- [x] **3B.6** `output/components/list.rs` — ordered/unordered, `TokenItem` items, per-item bullet/color override
- [x] **3B.7** `output/components/tabular_data.rs` — column-aligned grid, `first_column_subdued`, max-width calculation
- [x] **3B.8** `output/components/banner.rs` — `BannerType` enum, `BoxWithBorder` (rounded `╭╮╰╯`), `BoxWithTopBottomLines` (`──`), `Footnotes` block, `LinksContext` provider
- [x] **3B.9** `output/components/alert.rs` — `AlertProps` with rich `TokenItem` for headline/body/nextSteps/reference, `CustomSection` with TabularData support
- [x] **3B.10** `output/components/fatal_error.rs` — stack trace with `StackTracey`-style source lines, `ExternalError` tool display, markdown link detection, custom sections with TabularData
- [x] **3B.11** `output/components/table.rs` — `Table` + `Row` + `Column`: headers, `─` separator row, auto-width, per-column color, `ScalarDict` row type
- [x] **3B.12** All 3B tests pass, clippy clean

### 3C — Infrastructure Components (~800 LOC, 13 files)

Building blocks for interactive prompts and animated displays.

- [x] **3C.1** `output/components/scrollbar.rs` — visual scrollbar: `│` background, `║` position, `△`/`▽` arrows, proportional scrolling, no-color mode
- [x] **3C.2** `output/components/text_input.rs` — dual-mode: cursor movement, insert/delete, password masking, placeholder rendering (first char inverse, rest dim), Tab-fill. ANSI mode → inline `\x1b[7m` cursor. TUI mode → ratatui widget.
- [x] **3C.3** `output/components/loading_bar.rs` — animated bar: `hillString` pattern (`▁▁▂▃▄▅▆▇█`) + title + `...`. Rainbow gradient via TextAnimation. TTY detection. `noColor`/`noProgressBar` options.
- [x] **3C.4** `output/components/text_animation.rs` — rainbow HSV gradient animation, 35ms frame rate, `gradient-string`-style hue rotation, terminal resize handling
- [x] **3C.5** `output/components/prompts/prompt_layout.rs` — shared shell: `?` prefix + message, optional header/search bar, `InfoTable`, `InfoMessage`, input area, submitted state (green `✔` + answer). Dynamic height from terminal rows. `availableLines` calculation.
- [x] **3C.6** `output/components/prompts/info_table.rs` — `Record<string, Items[]>` or `InfoTableSection[]` with headers, colored bullets, helper text, empty state placeholder
- [x] **3C.7** `output/components/prompts/info_message.rs` — colored title + body block
- [x] **3C.8** `output/hooks/use_prompt.rs` — `PromptState` enum (`Idle`/`Loading`/`Submitted`/`Error`/`Cancelled`), `answer`, `setAnswer`, `setPromptState`
- [x] **3C.9** `output/hooks/use_select_state.rs` — `OptionMap<T>` (linked-list map with `first`, `next`, `prev`), `useSelectState` reducer with `selectNext`/`selectPrevious`/`selectOption`, pagination (`visibleFromIndex`/`visibleToIndex`), disabled option skipping
- [x] **3C.10** `output/hooks/use_layout.rs` — `Layout { two_thirds, one_third, full_width }`, terminal resize listener, `MIN_FULL_WIDTH = 20`, `MIN_FRACTION_WIDTH = 80`
- [x] **3C.11** `output/hooks/use_abort_signal.rs` — tokio channel-based abort listener, sets `isAborted`, triggers `onAbort` callback
- [x] **3C.12** `output/hooks/use_async_and_unmount.rs` — runs async task in tokio, calls `onFulfilled`/`onRejected`, signals completion
- [x] **3C.13** `output/hooks/use_exit_on_ctrl_c.rs` — Ctrl+C handler that calls `tree_kill(process.pid, 'SIGINT')`
- [x] **3C.14** All 3C tests pass, clippy clean

### 3D — Interactive Prompt Components (~1,100 LOC, 6 files)

Full prompt suite. All use `PromptLayout` + hooks internally. Each implements the `Prompt` trait with `render()` and `handle_event()`.

- [x] **3D.1** `output/components/prompts/select_input.rs` — `Item<T>` with `label`, `value`, `key` (shortcut), `group`, `helperText`, `disabled`. Group sorting per `groupOrder`. Shortcut key validation (all ≤1 char). Arrow navigation + shortcut keys + Enter submit. `loading`/`errorMessage`/`emptyMessage` states. `hasMorePages` + `morePagesMessage`. Scrollbar integration. `availableLines` limit (default 25).
- [x] **3D.2** `output/components/prompts/select_prompt.rs` — wraps `SelectInput` in `PromptLayout`. `message`, `choices`, `infoTable`, `infoMessage`, `defaultValue`, `abortSignal`, `groupOrder`. `onSubmit` → `complete()`.
- [x] **3D.3** `output/components/prompts/confirmation_prompt.rs` — thin wrapper: `SelectPrompt<bool>` with Yes/No choices. `confirmationMessage` (default `"Yes, confirm"`), `cancellationMessage` (default `"No, cancel"`), `defaultValue` (default `true`).
- [x] **3D.4** `output/components/prompts/autocomplete_prompt.rs` — search `TextInput` + debounced `search` callback + `SelectInput`. Default in-memory filter on `label`/`group` with `searchDebounceMs: 0`. Custom search with `searchDebounceMs: 400`. Loading state (100ms delay before showing). Error state on search failure. `hasMorePages` messaging. `MIN_NUMBER_OF_ITEMS_FOR_SEARCH = 5`.
- [x] **3D.5** `output/components/prompts/text_prompt.rs` — `TextInput` + validation + preview + `▔▔▔` underline. `allowEmpty`, `emptyDisplayedValue` (default `"(empty)"`), `defaultValue`, `password`, `validate`, `preview` callback. Submitted state: green `✔` + answer. Error state: red `>` + error text. `useLayout` → `oneThird` width.
- [x] **3D.6** `output/components/prompts/dangerous_confirmation_prompt.rs` — exact-string `TextInput` + `⚠ WARNING` banner in red + `InfoTable`. Escape → submit `false`. Value matches `confirmation` → submit `true`. Completed: green `✔ Confirmed` / red `✖ Cancelled`.
- [x] **3D.7** All 3D tests pass, clippy clean

### 3E — Streaming Components (~650 LOC, 5 files)

Components that maintain a render loop while work progresses. Processes write to `tokio::sync::mpsc` channels, chunks are collected and re-rendered incrementally.

- [x] **3E.1** `output/components/single_task.rs` — wraps one async task with `LoadingBar`. `updateStatus: (TokenizedString) => void` callback. Ctrl+C abort via `onAbort` or default `tree_kill`. `onComplete` → `complete()`.
- [x] **3E.2** `output/components/tasks.rs` — sequential task runner via `Task<TContext>` trait with `title` (string or `TokenizedString`), `task: (ctx, task) => Promise<void | Task[]>`, `retry`, `skip`, `errors`. Shared context passing. Shows `LoadingBar` for current task. `silent`/`noColor`/`noProgressBar`/`abortSignal` options. Subtask support.
- [x] **3E.3** `output/components/concurrent_output.rs` — `OutputProcess` with `prefix` + `action: (stdout, stderr, signal) => Promise<void>` callback model. `Writable`-equivalent channels per process. Color cycling (5 default / 6 alternative). `prefixColumnSize` (max 25). `showTimestamps` (`HH:MM:SS`). `keepRunningAfterProcessesResolve`. ANSI stripping per process. `ConcurrentOutputContext` for nested prefix overrides.
- [x] **3E.4** `output/components/static_component.rs` — Ink `<Static>` equivalent: sticky non-interactive output that persists across re-renders
- [x] **3E.5** `output/engine/streaming_loop.rs` — modified event loop: multiplexes keyboard input, channel data, timer ticks via `tokio::select!`. Exits when all processes resolve or any throws.
- [x] **3E.6** All 3E tests pass, clippy clean

### 3F — Public API & Test Infrastructure (~1,200 LOC, 12 files)

Wire everything into the public-facing API. Build test tooling.

- [x] **3F.1** `output/public_api.rs` — all 14 `render*` functions:
  - [x] `renderInfo(options) → String` — info banner via `renderOnce(<Alert type="info">)`
  - [x] `renderSuccess(options) → String` — success banner
  - [x] `renderWarning(options) → String` — warning banner
  - [x] `renderError(options) → String` — error banner
  - [x] `renderFatalError(error, options?) → String` — fatal error with stack trace
  - [x] `renderSelectPrompt(options) → Promise<T>` — interactive select
  - [x] `renderConfirmationPrompt(options) → Promise<bool>` — yes/no confirm
  - [x] `renderAutocompletePrompt(options) → Promise<T>` — search + select
  - [x] `renderTextPrompt(options) → Promise<String>` — free-text input
  - [x] `renderDangerousConfirmationPrompt(options) → Promise<bool>` — type-to-confirm
  - [x] `renderConcurrent(options) → Promise<void>` — streaming concurrent output
  - [x] `renderTasks(tasks, options?) → Promise<TContext>` — sequential task runner
  - [x] `renderSingleTask(options) → Promise<T>` — single task with loading bar
  - [x] `renderTable(options) → String` — table rendering
- [x] **3F.2** `output/public_api.rs` — output token factory and output functions:
  - [x] `outputToken` factory methods via `TokenItem` (raw, command, json, path, link, heading, subheading, italic, errorText, cyan, yellow, magenta, green, gray, successIcon, failIcon)
  - [x] `outputContent` — via `OutputContent` struct (legacy) and `TokenizedText` (new)
  - [x] `OutputMessage` type — via `impl Into<OutputContent>`
  - [x] `stringifyMessage(message) → String` — via `stringify_message()`
  - [x] `formatSection(title, body) → String` — via `public_api::format_section()`
  - [x] `unstyled(message) → String` — ANSI strip via `strip_ansi()`
  - [x] `shouldDisplayColors() → bool` — via `public_api::should_display_colors()`
  - [x] `collectedLogs`, `collectLog(key, content)`, `clearCollectedLogs()` — via `TestConsole`
- [x] **3F.3** `output/public_api.rs` — `TestConsole`: `frames[]`, `last_frame()`, `write(frame)`, `render_context()`
- [x] **3F.4** `output/engine/lifecycle.rs` — `render_static()` equivalent for one-shot rendering
- [x] **3F.5** `output/engine/streaming_loop.rs` — `run_streaming()` and `run_streaming_with_channel()` for interactive rendering
- [x] **3F.6** Tests: 15 tests in `public_api.rs`, comprehensive component test coverage across all files
- [x] **3F.7** All 3F tests pass, clippy clean

### 3G — Integration & Legacy Cleanup ✅

- [x] **3G.1** `output/mod.rs` — cleaned up, only new component system remains
- [x] **3G.2** `strip_ansi` moved from `token.rs` to `tokens/`; `token.rs` retained for `OutputContent` backward compat
- [x] **3G.3** Remove old `inflector.rs` — fully replaced by `tokens/` module (359 LOC removed)
- [x] **3G.4** Remove old `alert.rs` — replaced by `components/alert.rs`
- [x] **3G.5** Remove old `banner.rs` — replaced by `components/banner.rs`
- [x] **3G.6** Remove old `prompt.rs`, `concurrent_output.rs`, `tasks.rs`, `text_input.rs` — replaced by component versions
- [x] **3G.7** Remove old `link.rs`, `list.rs`, `table.rs` — replaced by component versions
- [x] **3G.8** Updated re-exports in `mod.rs` — `strip_ansi` sourced from `tokens/` module
- [x] **3G.9** `cargo clippy -D warnings` — zero warnings
- [x] **3G.10** `cargo test -p cli-kit` — 685 lib tests + 13 integration tests pass
- [x] **3G.11** PHASES.md updated with final numbers

---

## Phase 4 — Session OAuth Completion (+1,500 LOC)

### 4.1 `session/identity.rs` (16 → ~100 LOC)
- [x] JWT extraction: sub (user ID), exp, scopes
- [x] ID token validation with JWKS
- [x] Match upstream `identity.ts` (69 LOC)

### 4.2 `session/device_authorization.rs` (93 → ~200 LOC)
- [x] Complete polling loop: `/oauth/device/code` → poll `/oauth/device/token`
- [x] Timeout handling
- [x] Cancellation support
- [x] Resume from stored `PendingDeviceAuth`
- [x] Match upstream `device-authorization.ts` (190 LOC)

### 4.3 `session/exchange.rs` (294 → ~500 LOC)
- [x] `exchange_custom_partner_token()` — env → Partners API token
- [x] `exchange_app_automation_token_for_app_management()`
- [x] `exchange_app_automation_token_for_business_platform()`
- [x] `exchange_identity_token_for_api_token()` — identity → API surface
- [x] Match upstream `exchange.ts` (287 LOC)

### 4.4 `session/validate.rs` (240 → ~350 LOC)
- [x] Scope comparison: requested ⊆ stored
- [x] `needs_refresh()` with `SESSION_EXPIRATION_MARGIN_MINUTES`
- [x] Match upstream `validate.ts` (80 LOC) + `scopes.ts` (87 LOC)

### 4.5 `session/store.rs` (135 → ~250 LOC)
- [x] Multi-session support (multiple stores, orgs)
- [x] Keychain integration (Linux secret-service)
- [x] Match upstream `store.ts` (107 LOC)

### 4.6 `session/scopes.rs` (71 → ~150 LOC)
- [x] `scope_string()`, `scope_set()`, `scope_intersection()`, `scope_difference()`
- [x] Match upstream `scopes.ts` (87 LOC)

### 4.7 `session/schema.rs` (31 → ~100 LOC)
- [x] Full session type: exchange, identity, expires, scopes
- [x] Match upstream `schema.ts` (78 LOC)

### 4.8 `session/mod.rs` (411 → ~800 LOC)
- [x] `ensure_authenticated_partners()`
- [x] `ensure_authenticated_admin()`
- [x] `ensure_authenticated_themes()`
- [x] `ensure_authenticated_app_management_and_business_platform()`
- [x] `ensure_authenticated_storefront()`
- [x] `ensure_authenticated_business_platform()`
- [x] `ensure_authenticated_admin_as_app()`
- [x] Each: env check → store → OAuth fallback
- [x] Match upstream `session.ts` (357 LOC) + `session-prompt.ts` (115 LOC)

---

## Phase 5 — Analytics + Error Reporting (+1,000 LOC)

### 5.1 `util/analytics.rs` (89 → ~500 LOC)
- [x] Monorail event schema: `cli/command_exec/1.0`, `cli/command_abort/1.0`
- [x] Event types: CommandExec, CommandAbort, FatalError
- [x] Batching: 30s interval or 100 events
- [x] Retry on flush failure (exponential backoff)
- [x] Anonymous vs authenticated (hashable identity)
- [x] Metadata: version, OS, platform, project, command, duration
- [x] Match upstream `monorail.ts` (265 LOC) + `private/node/analytics/` (~740 LOC)

### 5.2 `util/error_handler.rs` (new, ~300 LOC)
- [x] Catch panics, format stack trace
- [x] Environment context collection
- [x] Rate-limited (max 1/min)
- [x] Match upstream `error-handler.ts` (318 LOC) + `error-categorizer.ts` (114 LOC)

### 5.3 `constants.rs` — new endpoints
- [x] `MONORAIL_ENDPOINT`
- [x] `ERROR_ANALYTICS_ENDPOINT`

---

## Phase 6 — Systems & Utilities (+2,500 LOC)

### 6.1 `util/system.rs` (156 → ~500 LOC)
- [x] `execute_subprocess()` — spawn + capture output, timeout
- [x] `open_url()` — `xdg-open` / system open
- [x] `tree_kill()` — kill process tree
- [x] `check_port()` — TCP port availability
- [x] `is_global_installation()` — detect global install
- [x] Match upstream `system.ts` (411 LOC) + `tree-kill.ts` (230 LOC) + `tcp.ts` (111 LOC)

### 6.2 `util/fs.rs` (129 → ~400 LOC)
- [x] `archive_zip()` / `extract_tar()` — port `archiver.ts` (203 LOC)
- [x] `find_up()` — walk up directories
- [x] `is_hidden()` / `hidden_folder()`
- [x] `glob()` — file pattern matching
- [x] Extended file ops (move, copy with permissions)
- [x] Match upstream `fs.ts` (727 LOC)

### 6.3 `util/github.rs` (108 → ~250 LOC)
- [x] `download_github_release()` — fetch release artifacts
- [x] `get_latest_github_release()` — check releases API
- [x] Match upstream `github.ts` (174 LOC)

### 6.4 `util/dot_env.rs` (new, ~200 LOC)
- [x] `.env` file parsing, variable substitution
- [x] Local env detection (Spin, Codespaces, Gitpod, CloudShell)
- [x] Match upstream `dot-env.ts` (143 LOC) + `context/local.ts` (328 LOC)

### 6.5 `util/path.rs` (new, ~250 LOC)
- [x] Cross-platform path resolution
- [x] Temp directory management
- [x] Match upstream `path.ts` (236 LOC) + `temp-dir.ts` (8 LOC)

### 6.6 `util/result.rs` (new, ~150 LOC)
- [x] Result-like monad for nullable JS patterns
- [x] Match upstream `result.ts` (145 LOC)

### 6.7 `util/package_manager.rs` (84 → ~300 LOC)
- [x] `detect_package_manager()` — lockfile + global detection
- [x] `install_packages()` — run install
- [x] `add_packages()` — add deps
- [x] `run_script()` — execute scripts
- [x] `get_package_version()` — read version
- [x] Workspace support (pnpm, npm workspaces)
- [x] Match upstream `node-package-manager.ts` (798 LOC) — subset needed

### 6.8 `util/context.rs` (23 → ~100 LOC)
- [x] Deprecations store
- [x] Service context
- [x] Context utilities
- [x] Match upstream `private/node/context/` (~150 LOC total)

### 6.9 New files
- [x] `util/framework.rs` — framework detection (~200 LOC, upstream `framework.ts` 199 LOC)
- [x] `util/import_extractor.rs` — import scanning (~270 LOC, upstream `import-extractor.ts` 270 LOC)
- [x] `util/request_ids.rs` — x-request-id tracking (~50 LOC, upstream `request-ids.ts` 43 LOC)

---

## Phase 7 — CLI Commands (+1,000 LOC)

### 7.1 `commands/cache/clear.rs`
- [x] Remove all cached files from `cache_path()`

### 7.2 `commands/config/autoupgrade/{on,off,status}.rs`
- [x] Toggle auto-upgrade in config store
- [x] Show current status

### 7.3 `commands/upgrade.rs`
- [x] Check latest version on GitHub
- [x] Download + install (cargo install or binary)
- [x] Match upstream `upgrade.ts` (299 LOC)

### 7.4 `commands/search.rs`
- [x] Search docs.shopify.com
- [x] Match upstream `cli/services/commands/search.ts`

### 7.5 `commands/notifications/{list,generate}.rs`
- [x] List/generate notifications
- [x] Match upstream `notifications-system.ts` (323 LOC)

### 7.6 `commands/kitchen_sink/{static,async,prompts}.rs`
- [x] UI component demos
- [x] Match upstream `services/kitchen-sink/`

### 7.7 `commands/debug/command_flags.rs`
- [x] Show parsed flags for debugging

---

## Phase 8 — Theme Crate (+3,500 LOC)

New crate `crates/theme/`. Details in PORT.md §5.3.
Commands: `push`, `pull`, `dev`, `delete`, `list`, `info`, `open`, `share`, `check`.

---

## Phase 9 — App Crate (+5,000 LOC)

New crate `crates/app/`. Details in PORT.md §5.2.
Commands: `dev`, `deploy`, `build`, `init`, `generate`, `env`, `config`, `function`, `logs`, `versions`, `webhook`, `release`, `bulk`.

---

## Phase 10 — Remaining Domain Crates (+3,000 LOC)

- [x] `store` crate — `store create` command
- [x] `organizations` crate — org selection
- [x] `plugin-cloudflare` crate — tunnel management
- [x] `plugin-did-you-mean` crate — command autocorrection
- [x] `cli` crate — main binary wiring (update for new commands)
- [x] `create-app` crate — standalone binary

---

## Phase 11 — Polish & Release (+1,500 LOC)

- [x] `cargo doc` — fix all doc warnings
- [x] Full integration test suite — every command
- [x] Cross-compilation test (musl, Windows, macOS)
- [x] CI pipeline hardening
- [x] Performance profiling + optimization
- [x] README + contribution docs
- [x] Version alignment with upstream
