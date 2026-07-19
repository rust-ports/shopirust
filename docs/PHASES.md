# CLI-Kit Phase Plan & Checklist

**Current**: ~10,500 LOC / 56 files / ~35% of upstream cli-kit
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

## Phase 3 — UI System (ratatui rewrite, ~5,650 LOC)

**Architecture**: ratatui-based Ink-equivalent engine. Every component renders to either `RenderMode::Ansi` (static colored string for non-TTY/tests) or `RenderMode::Tui` (ratatui Frame for interactive TTY). Engine runs event loop via crossterm + tokio. No virtual DOM — canvas-style redraw. Static components (Banner, Alert, List, Link, Table, FatalError) stay as `colored` ANSI functions; only interactive (prompts, text input) and streaming (concurrent output, tasks) use ratatui.

### 3A — Foundation (~900 LOC, 15 files)

- [ ] **3A.1** Add deps: ratatui, crossterm, tokio to Cargo.toml
- [ ] **3A.2** `output/engine/mod.rs` — `RenderMode`, `RenderContext`, `RenderFragment` (Ansi/Span split), `Component` trait
- [ ] **3A.3** `output/engine/event_loop.rs` — crossterm event stream → `Event` enum, dispatch to component, re-render cycle
- [ ] **3A.4** `output/engine/lifecycle.rs` — `run_prompt()` (interactive loop), `run_streaming()` (channel-based loop), `render_static()` (one-shot String)
- [ ] **3A.5** `output/engine/contexts.rs` — `CompletionContext`, `LinksContext` passed through `RenderContext`
- [ ] **3A.6** `output/engine/layout.rs` — `calculate_layout()` → `Layout { two_thirds, one_third, full_width }`
- [ ] **3A.7** `output/tokens/mod.rs` — `ContentToken<T>` trait, unify old token.rs + inflector.rs into one system
- [ ] **3A.8** `output/tokens/token_item.rs` — `Token` UN, `TokenItem<T>`, `InlineToken`, `BoldToken`, `LinkToken`, `ListToken` types matching upstream discriminated unions
- [ ] **3A.9** `output/tokens/tokenized_text.rs` — `TokenizedText` rendering engine: block/inline splitting, markdown link detection (`[label](url)` + `<url>`), dispatch to sub-components
- [ ] **3A.10** `output/tokens/lines_diff.rs` — `LinesDiffContentToken` rendering `Change[]` as green `+`/magenta `-`
- [ ] **3A.11** `output/colors.rs` — Color function wrappers (cyan, gray, magentaBright, etc.)
- [ ] **3A.12** `output/figures.rs` — Unicode symbols: `✔`, `✖`, `•`, `◆`, `‖`, `─`, `△`, `▽`, `■`, `▔`, `│`, `║`, `◉`, `→`
- [ ] **3A.13** `output/utilities.rs` — `message_with_punctuation()` helper
- [ ] **3A.14** Remove old `token.rs`, `inflector.rs` — replaced by `tokens/` module
- [ ] **3A.15** All 3A tests pass, clippy clean

### 3B — Static Components (~700 LOC, 12 files)

Small rendering-only components. Each produces `RenderFragment` (ANSI string or ratatui Span). Parent components compose them.

- [ ] **3B.1** `output/components/command.rs` — `` `{command}` `` in magentaBright
- [ ] **3B.2** `output/components/user_input.rs` — `{text}` in cyan
- [ ] **3B.3** `output/components/subdued.rs` — `{text}` in dim
- [ ] **3B.4** `output/components/file_path.rs` — `{path}` in italic
- [ ] **3B.5** `output/components/link.rs` — hyperlink `\x1b]8;;` vs `label (url)` vs footnote `[N]`, `LinksContext` integration
- [ ] **3B.6** `output/components/list.rs` — ordered/unordered, `TokenItem` items, per-item bullet/color override
- [ ] **3B.7** `output/components/tabular_data.rs` — column-aligned grid, `first_column_subdued`, max-width calculation
- [ ] **3B.8** `output/components/banner.rs` — `BannerType` enum, `BoxWithBorder` (rounded `╭╮╰╯`), `BoxWithTopBottomLines` (`──`), `Footnotes` block, `LinksContext` provider
- [ ] **3B.9** `output/components/alert.rs` — `AlertProps` with rich `TokenItem` for headline/body/nextSteps/reference, `CustomSection` with TabularData support
- [ ] **3B.10** `output/components/fatal_error.rs` — stack trace with `StackTracey`-style source lines, `ExternalError` tool display, markdown link detection, custom sections with TabularData
- [ ] **3B.11** `output/components/table.rs` — `Table` + `Row` + `Column`: headers, `─` separator row, auto-width, per-column color, `ScalarDict` row type
- [ ] **3B.12** All 3B tests pass, clippy clean

### 3C — Infrastructure Components (~800 LOC, 13 files)

Building blocks for interactive prompts and animated displays.

- [ ] **3C.1** `output/components/scrollbar.rs` — visual scrollbar: `│` background, `║` position, `△`/`▽` arrows, proportional scrolling, no-color mode
- [ ] **3C.2** `output/components/text_input.rs` — dual-mode: cursor movement, insert/delete, password masking, placeholder rendering (first char inverse, rest dim), Tab-fill. ANSI mode → inline `\x1b[7m` cursor. TUI mode → ratatui widget.
- [ ] **3C.3** `output/components/loading_bar.rs` — animated bar: `hillString` pattern (`▁▁▂▃▄▅▆▇█`) + title + `...`. Rainbow gradient via TextAnimation. TTY detection. `noColor`/`noProgressBar` options.
- [ ] **3C.4** `output/components/text_animation.rs` — rainbow HSV gradient animation, 35ms frame rate, `gradient-string`-style hue rotation, terminal resize handling
- [ ] **3C.5** `output/components/prompts/prompt_layout.rs` — shared shell: `?` prefix + message, optional header/search bar, `InfoTable`, `InfoMessage`, input area, submitted state (green `✔` + answer). Dynamic height from terminal rows. `availableLines` calculation.
- [ ] **3C.6** `output/components/prompts/info_table.rs` — `Record<string, Items[]>` or `InfoTableSection[]` with headers, colored bullets, helper text, empty state placeholder
- [ ] **3C.7** `output/components/prompts/info_message.rs` — colored title + body block
- [ ] **3C.8** `output/hooks/use_prompt.rs` — `PromptState` enum (`Idle`/`Loading`/`Submitted`/`Error`/`Cancelled`), `answer`, `setAnswer`, `setPromptState`
- [ ] **3C.9** `output/hooks/use_select_state.rs` — `OptionMap<T>` (linked-list map with `first`, `next`, `prev`), `useSelectState` reducer with `selectNext`/`selectPrevious`/`selectOption`, pagination (`visibleFromIndex`/`visibleToIndex`), disabled option skipping
- [ ] **3C.10** `output/hooks/use_layout.rs` — `Layout { two_thirds, one_third, full_width }`, terminal resize listener, `MIN_FULL_WIDTH = 20`, `MIN_FRACTION_WIDTH = 80`
- [ ] **3C.11** `output/hooks/use_abort_signal.rs` — tokio channel-based abort listener, sets `isAborted`, triggers `onAbort` callback
- [ ] **3C.12** `output/hooks/use_async_and_unmount.rs` — runs async task in tokio, calls `onFulfilled`/`onRejected`, signals completion
- [ ] **3C.13** `output/hooks/use_exit_on_ctrl_c.rs` — Ctrl+C handler that calls `tree_kill(process.pid, 'SIGINT')`
- [ ] **3C.14** All 3C tests pass, clippy clean

### 3D — Interactive Prompt Components (~1,100 LOC, 6 files)

Full prompt suite. All use `PromptLayout` + hooks internally. Each implements the `Prompt` trait with `render()` and `handle_event()`.

- [ ] **3D.1** `output/components/prompts/select_input.rs` — `Item<T>` with `label`, `value`, `key` (shortcut), `group`, `helperText`, `disabled`. Group sorting per `groupOrder`. Shortcut key validation (all ≤1 char). Arrow navigation + shortcut keys + Enter submit. `loading`/`errorMessage`/`emptyMessage` states. `hasMorePages` + `morePagesMessage`. Scrollbar integration. `availableLines` limit (default 25).
- [ ] **3D.2** `output/components/prompts/select_prompt.rs` — wraps `SelectInput` in `PromptLayout`. `message`, `choices`, `infoTable`, `infoMessage`, `defaultValue`, `abortSignal`, `groupOrder`. `onSubmit` → `complete()`.
- [ ] **3D.3** `output/components/prompts/confirmation_prompt.rs` — thin wrapper: `SelectPrompt<bool>` with Yes/No choices. `confirmationMessage` (default `"Yes, confirm"`), `cancellationMessage` (default `"No, cancel"`), `defaultValue` (default `true`).
- [ ] **3D.4** `output/components/prompts/autocomplete_prompt.rs` — search `TextInput` + debounced `search` callback + `SelectInput`. Default in-memory filter on `label`/`group` with `searchDebounceMs: 0`. Custom search with `searchDebounceMs: 400`. Loading state (100ms delay before showing). Error state on search failure. `hasMorePages` messaging. `MIN_NUMBER_OF_ITEMS_FOR_SEARCH = 5`.
- [ ] **3D.5** `output/components/prompts/text_prompt.rs` — `TextInput` + validation + preview + `▔▔▔` underline. `allowEmpty`, `emptyDisplayedValue` (default `"(empty)"`), `defaultValue`, `password`, `validate`, `preview` callback. Submitted state: green `✔` + answer. Error state: red `>` + error text. `useLayout` → `oneThird` width.
- [ ] **3D.6** `output/components/prompts/dangerous_confirmation_prompt.rs` — exact-string `TextInput` + `⚠ WARNING` banner in red + `InfoTable`. Escape → submit `false`. Value matches `confirmation` → submit `true`. Completed: green `✔ Confirmed` / red `✖ Cancelled`.
- [ ] **3D.7** All 3D tests pass, clippy clean

### 3E — Streaming Components (~650 LOC, 5 files)

Components that maintain a render loop while work progresses. Processes write to `tokio::sync::mpsc` channels, chunks are collected and re-rendered incrementally.

- [ ] **3E.1** `output/components/single_task.rs` — wraps one async task with `LoadingBar`. `updateStatus: (TokenizedString) => void` callback. Ctrl+C abort via `onAbort` or default `tree_kill`. `onComplete` → `complete()`.
- [ ] **3E.2** `output/components/tasks.rs` — sequential task runner via `Task<TContext>` trait with `title` (string or `TokenizedString`), `task: (ctx, task) => Promise<void | Task[]>`, `retry`, `skip`, `errors`. Shared context passing. Shows `LoadingBar` for current task. `silent`/`noColor`/`noProgressBar`/`abortSignal` options. Subtask support.
- [ ] **3E.3** `output/components/concurrent_output.rs` — `OutputProcess` with `prefix` + `action: (stdout, stderr, signal) => Promise<void>` callback model. `Writable`-equivalent channels per process. Color cycling (5 default / 6 alternative). `prefixColumnSize` (max 25). `showTimestamps` (`HH:MM:SS`). `keepRunningAfterProcessesResolve`. ANSI stripping per process. `ConcurrentOutputContext` for nested prefix overrides.
- [ ] **3E.4** `output/components/static_component.rs` — Ink `<Static>` equivalent: sticky non-interactive output that persists across re-renders
- [ ] **3E.5** `output/engine/streaming_loop.rs` — modified event loop: multiplexes keyboard input, channel data, timer ticks via `tokio::select!`. Exits when all processes resolve or any throws.
- [ ] **3E.6** All 3E tests pass, clippy clean

### 3F — Public API & Test Infrastructure (~1,200 LOC, 12 files)

Wire everything into the public-facing API. Build test tooling.

- [ ] **3F.1** `output/public_api.rs` — all 14 `render*` functions:
  - [ ] `renderInfo(options) → String` — info banner via `renderOnce(<Alert type="info">)`
  - [ ] `renderSuccess(options) → String` — success banner
  - [ ] `renderWarning(options) → String` — warning banner
  - [ ] `renderError(options) → String` — error banner
  - [ ] `renderFatalError(error, options?) → String` — fatal error with stack trace
  - [ ] `renderSelectPrompt(options) → Promise<T>` — interactive select
  - [ ] `renderConfirmationPrompt(options) → Promise<bool>` — yes/no confirm
  - [ ] `renderAutocompletePrompt(options) → Promise<T>` — search + select
  - [ ] `renderTextPrompt(options) → Promise<String>` — free-text input
  - [ ] `renderDangerousConfirmationPrompt(options) → Promise<bool>` — type-to-confirm
  - [ ] `renderConcurrent(options) → Promise<void>` — streaming concurrent output
  - [ ] `renderTasks(tasks, options?) → Promise<TContext>` — sequential task runner
  - [ ] `renderSingleTask(options) → Promise<T>` — single task with loading bar
  - [ ] `renderTable(options) → String` — table rendering
- [ ] **3F.2** `output/output_api.rs` — output token factory and output functions:
  - [ ] `outputToken` factory with all 17 methods: `raw`, `genericShellCommand`, `json`, `path`, `link`, `heading`, `subheading`, `italic`, `errorText`, `cyan`, `yellow`, `magenta`, `green`, `gray`, `packagejsonScript`, `successIcon` (green `✔`), `failIcon` (`✖`), `linesDiff`
  - [ ] `outputContent` — tagged-template equivalent (macro or builder)
  - [ ] `OutputMessage` type (`String | TokenizedString`)
  - [ ] `TokenizedString` class
  - [ ] `stringifyMessage(message) → String`
  - [ ] `itemToString(item) → String`
  - [ ] `outputInfo`, `outputSuccess`, `outputWarn`, `outputDebug`, `outputResult`, `outputNewline`
  - [ ] `formatSection(title, body) → String`
  - [ ] `unstyled(message) → String` — ANSI strip
  - [ ] `shouldDisplayColors() → bool` — memoized
  - [ ] `collectedLogs`, `collectLog(key, content)`, `clearCollectedLogs()` — test infra
  - [ ] `logLevelValue(level)`, `currentLogLevel()`, `shouldOutput(level)` — log gating
- [ ] **3F.3** `output/engine/stdout_mock.rs` — `Stdout` mock: `EventEmitter`, `frames[]`, `lastFrame()`, `columns`/`rows`, `write(frame)`
- [ ] **3F.4** `output/engine/render_once.rs` — `renderOnce(element)` → renders to `Stdout`, returns `lastFrame()`, used by tests and static render* functions
- [ ] **3F.5** `output/engine/render.rs` — `render(element)` → wraps in `InkLifecycleRoot`, runs event loop, waits for `waitUntilExit()`
- [ ] **3F.6** Tests for all 26 components using `renderOnce` + simulated key events (minimum 5 tests per component)
- [ ] **3F.7** All 3F tests pass, clippy clean

### 3G — Integration & Legacy Cleanup (~300 LOC, 8 files)

- [ ] **3G.1** `output/mod.rs` — wire everything, public re-exports match upstream `output.ts` + `ui.tsx`
- [ ] **3G.2** Remove old `token.rs` — functionality absorbed by `tokens/` module
- [ ] **3G.3** Remove old `inflector.rs` — absorbed by `tokens/` module
- [ ] **3G.4** Remove old `alert.rs` — replaced by `components/alert.rs`
- [ ] **3G.5** Remove old `banner.rs` inline render functions — replaced by `components/banner.rs`
- [ ] **3G.6** Deprecate old `prompt.rs` — keep as non-TTY fallback path, mark deprecated
- [ ] **3G.7** Update `link.rs`, `list.rs`, `table.rs` to go through new component system
- [ ] **3G.8** Update all callers of old output functions throughout crates
- [ ] **3G.9** Final `cargo clippy -D warnings` — zero warnings
- [ ] **3G.10** Final `cargo test` — all tests pass across all crates
- [ ] **3G.11** Update `docs/PHASES.md` with final numbers

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
