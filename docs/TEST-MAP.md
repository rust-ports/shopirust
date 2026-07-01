# Shopify CLI Test Coverage Map

> **Counterpart to CLI-MAP.md and API-shopify.md.**
> Documents the entire test landscape: infrastructure, API surface coverage, helper coverage, command coverage, E2E scenarios, test utilities, and coverage gaps.

---

## 1. Test Infrastructure

### Test Runner

| Layer | Runner | Config |
|-------|--------|--------|
| Unit/Integration | [Vitest](https://vitest.dev) | Per-package `vitest.config.ts` + root `vite.config.ts` for aliases |
| End-to-End | Mocha + Chai | `packages/e2e/.mocha.json` |

### Key Configuration Files

| File | Purpose |
|------|---------|
| `vite.config.ts` (root) | Module aliases: `@shopify/cli-kit` → `packages/cli-kit/src`, `@shopify/app` → `packages/app/src`, `@shopify/theme` → `packages/theme/src` |
| `packages/cli-kit/vitest.config.ts` | Vitest setup for cli-kit |
| `packages/app/vitest.config.ts` | Vitest setup for app package |
| `packages/theme/vitest.config.ts` | Vitest setup for theme package |
| `packages/e2e/package.json` | E2E test script configuration |

### Test Scripts (per `package.json`)

```jsonc
// Typical pattern across packages:
"test": "vitest run",
"test:watch": "vitest"
```

### Module Aliases (Resolution)

Tests import via `@shopify/cli-kit`, `@shopify/app`, `@shopify/theme` — same as production. The vite config aliases resolve these to `src/` directories. This means:

- Tests exercise the **same import paths** as production code
- No separate `dist/` compilation needed during testing
- Mock paths must use the alias (e.g., `vi.mock('@shopify/cli-kit/node/api/partners')`)

### Mocking Strategy

- **Primary**: `vi.mock()` at module level, `vi.fn()` for individual functions
- **Pattern**: Mock the imported module, then use `vi.mocked(fn).mockResolvedValue()` per-test
- **Session mocking**: Test fixtures with `testPartnersUserSession()`, `testApp()`
- **No DI container** — mocking relies entirely on Vitest's hoisted module mocking

---

## 2. API Surface Test Coverage

References the 9 API surfaces catalogued in `API-shopify.md`.

### 2.1 Partners API (`@shopify/cli-kit/node/api/partners`)

| Aspect | Status |
|--------|--------|
| API functions (`partnersRequest`, `partnersRequestDoc`) | ❌ No direct tests |
| `PartnersClient` class (app package) | ✅ **1 file** — `utilities/developer-platform-client/partners-client.test.ts` (239 lines) |

`partners-client.test.ts` covers:
- `createApp()`, `orgs()`, `orgFromId()`, `appFromId()`, `appsForOrg()`, `storesByOrg()`, `appExtensionRegistrations()`, `appVersions()`, `updateAppUrl()` — via mock
- Uses `vi.mock('@shopify/cli-kit/node/api/partners')` — **does not test the actual HTTP/graphql call**

### 2.2 Admin API (`@shopify/cli-kit/node/api/graphql` for Admin)

| Aspect | Status |
|--------|--------|
| Low-level `graphqlRequestDoc` for Admin | ✅ Indirect via `admin-as-app.test.ts` (69 lines) |
| `graphqlRequestDoc` for Admin | ✅ Also via `cli-kit/src/public/node/graphql.test.ts` |

`admin-as-app.test.ts` covers:
- `adminAsAppRequestDoc()` — verifies correct URL construction (`https://{store}/admin/api/unstable/graphql.json`)
- Token passing, variable handling
- Mocks `graphqlRequestDoc` from `@shopify/cli-kit/node/api/graphql`

### 2.3 Storefront Renderer API

| Aspect | Status |
|--------|--------|
| `storefrontRenderRequest()`/`storefrontRenderRequestDoc()` | ❌ **No tests** |

### 2.4 Function Runner API

| Aspect | Status |
|--------|--------|
| Function runner client | ❌ **No API-level tests** |

Note: `services/function/runner.test.ts` tests the *service layer*, not the API client itself.

### 2.5 App Management API

| Aspect | Status |
|--------|--------|
| App Management API client | ❌ **No tests** |

### 2.6 Business Platform APIs

| Aspect | Status |
|--------|--------|
| `business-platform-organizations/` queries | ❌ **No tests** |
| `business-platform-destinations/` queries | ❌ **No tests** |

### 2.7 GraphQL Layer (35 query/mutation files)

All under `packages/app/src/cli/api/graphql/`:

```
admin/                          ❌
all_app_extension_registrations ❌
app_active_version              ❌
app_deploy                      ❌
app_release                     ❌
app_version_by_tag              ❌
app_versions_diff               ❌
app-dev/                        ❌
app-management/                 ❌
bulk-operations/                ❌
convert_dev_to_transfer…        ❌
create_app                      ❌
current_account_info            ❌
development_preview             ❌
extension_create                ❌
extension_migrate_app_module    ❌
extension_migrate_flow_ext      ❌
extension_migrate_to_ui_ext     ❌
extension_specifications        ❌
find_app_preview_mode           ❌
find_app                        ❌
find_org_basic                  ❌
find_org                        ❌
find_store_by_domain            ❌
functions/                      ❌
generate_signed_upload_url      ❌
get_variant_id                  ❌
get_versions_list               ❌
subscribe_to_app_logs           ❌
template_specifications         ❌
update_urls                     ❌
webhooks/                       ❌
```

**Every single graphql query/mutation file lacks a dedicated test.**

### 2.8 Low-Level GraphQL Client (`cli-kit`)

| File | Status |
|------|--------|
| `packages/cli-kit/src/public/node/graphql.test.ts` | ✅ Tests `graphqlRequestDoc` with mocked HTTP |

---

## 3. Helper Function Test Coverage

### 3.1 cli-kit `/public/node/` (42 test files)

| Module | Test File | Coverage |
|--------|-----------|----------|
| `session` | `session.test.ts`, `session-prompt.test.ts` | ✅ Good — token store, auth flow, session prompts |
| `analytics` | `analytics.test.ts` | ✅ Good — event tracking, identity |
| `output` | `output.test.ts` | ✅ Good — formatting, streaming |
| `fs` | `fs.test.ts` | ✅ Good — file ops, in memory |
| `git` | `git.test.ts` | ✅ Good — clone, commit, status |
| `http` | `http.test.ts` | ✅ Good — fetch wrapper, timeout |
| `system` | `system.test.ts` | ✅ Good — exec, spawn |
| `environment` | `environment.test.ts`, `environments.test.ts` | ✅ Good — env detection, variables |
| `error-handler` | `error-handler.test.ts` | ✅ Good — error classification, reporting |
| `ui` | `ui.test.ts` | ✅ Good — prompt/select rendering |
| `node-package-manager` | `node-package-manager.test.ts` | ✅ Good — npm/yarn/pnpm detection |
| `plugins` | `plugins.test.ts` | ✅ Good — plugin loading |
| `liquid` | `liquid.test.ts` | ✅ Good — template rendering |
| `path` | `path.test.ts` | ✅ Good — path resolution |
| `os` | `os.test.ts` | ✅ Good — platform detection |
| `crypto` | `crypto.test.ts` | ✅ Good — hashing, keygen |
| `dot-env` | `dot-env.test.ts` | ✅ Good — env file parsing |
| `error` | `error.test.ts` | ✅ Good — abort, bug, cancel |
| `github` | `github.test.ts` | ✅ Good — GitHub API client |
| `graphql` | `graphql.test.ts` | ✅ Good — low-level client |
| `local-storage` | `local-storage.test.ts` | ✅ Good — KV store |
| `result` | `result.test.ts` | ✅ Good — Result type |
| `base-command` | `base-command.test.ts` | ✅ Good — command lifecycle |
| `cli-launcher` | `cli-launcher.test.ts` | ✅ Good — binary launcher |
| `archiver` | `archiver.integration.test.ts` | ✅ Integration — zip/unzip |
| `framework` | `framework.test.ts` | ✅ Good — framework detection |
| `metadata` | `metadata.test.ts` | ✅ Good — metadata collection |
| `system` | `system.test.ts`, `tcp.test.ts`, `tcp-retry.test.ts` | ✅ Good — TCP utilities |
| `monorail` | `monorail.test.ts` | ✅ Good — monorail events |
| `upgrade` | `upgrade.test.ts` | ✅ Good — version upgrade |
| `version` | `version.test.ts` | ✅ Good — version check |
| `tree-kill` | `tree-kill.test.ts` | ✅ Good — process tree kill |
| `schema` | `schema.test.ts` | ✅ Good — schema validation |
| `mimes` | `mimes.test.ts` | ✅ Good — MIME type mapping |
| `notifications-system` | `notifications-system.test.ts` | ✅ Good — desktop notifications |
| `json-schema` | `json-schema.test.ts` | ✅ Good — JSON Schema ops |
| `import-extractor` | `import-extractor.test.ts` | ✅ Good — import analysis |
| `serial-batch-processor` | `serial-batch-processor.test.ts` | ✅ Good — batch processing |
| `cli` | `cli.test.ts` | ✅ Good — CLI entrypoint |
| `hidden-folder` | `hidden-folder.test.ts` | ✅ Good — `.shopify` dir |
| `vscode` | `vscode.test.ts` | ✅ Good — VS Code integration |
| `is-global` | `is-global.test.ts` | ✅ Good — global check |

### 3.2 cli-kit `/public/common/` (10 test files)

| Module | Test File | Coverage |
|--------|-----------|----------|
| `string` | `string.test.ts` | ✅ Good |
| `object` | `object.test.ts` | ✅ Good |
| `array` | `array.test.ts` | ✅ Good |
| `collection` | `collection.test.ts` | ✅ Good |
| `url` | `url.test.ts` | ✅ Good |
| `retry` | `retry.test.ts` | ✅ Good |
| `function` | `function.test.ts` | ✅ Good |
| `lang` | `lang.test.ts` | ✅ Good |
| `gid` | `gid.test.ts` | ✅ Good |
| `json` | `json.test.ts` | ✅ Good |

### 3.3 Services with Tests (app package)

| Service | Test File(s) | Coverage |
|---------|-------------|----------|
| Build | `build/extension.test.ts`, `build/client-steps.test.ts`, `build/bundle-size.test.ts` | ✅ Good |
| Deploy | `deploy/*.test.ts` (multiple) | ✅ Good |
| Dev | `dev/*.test.ts` (multiple) | ✅ Good — fetch, process, graphql |
| Function | `function/build.test.ts`, `function/runner.test.ts`, `function/info.test.ts`, `function/replay.test.ts`, `function/schema-version.test.ts`, `function/common.test.ts`, `function/binaries.test.ts` | ✅ Good |
| Flow | `flow/serialize-partners-fields.test.ts` | ✅ Partial |
| Context | `context/identifiers-extensions.test.ts` | ✅ Good |

### 3.4 Services with Tests (theme package)

| Service | Test File(s) | Coverage |
|---------|-------------|----------|
| `open` | `open.test.ts` | ✅ |
| `delete` | `delete.test.ts` | ✅ |
| `package` | `package.test.ts` | ✅ |
| `pull` | `pull.test.ts` | ✅ |
| `push` | `push.test.ts` | ✅ |
| `dev` | `dev.test.ts` | ✅ |
| `list` | `list.test.ts` | ✅ |
| `publish` | `publish.test.ts` | ✅ |
| `rename` | `rename.test.ts` | ✅ |
| `duplicate` | `duplicate.test.ts` | ✅ |
| `init` | `init.test.ts` | ✅ |
| `info` | `info.test.ts` | ✅ |
| `check` | `check.test.ts` | ✅ |
| `console` | `console.test.ts` | ✅ |
| `profile` | `profile.test.ts` | ✅ |
| `local-storage` | `local-storage.test.ts` | ✅ |
| `dev-override` | `dev-override.test.ts` | ✅ |
| `metafields-pull` | `metafields-pull.test.ts` | ✅ |

Theme services have surprisingly good test coverage across 18 files.

---

## 4. Command Test Coverage

### 4.1 App Commands

| Command | Test | Coverage |
|---------|------|----------|
| `app/env/pull` | ✅ `commands/app/env/pull.test.ts` | ✅ |
| `app/config/validate` | ✅ `commands/app/config/validate.test.ts` | ✅ |
| `app/init` | ✅ `commands/app/init.test.ts` | ✅ |
| `organization/list` | ✅ `commands/organization/list.test.ts` | ✅ |
| `app/deploy` | ❌ No direct command test | Tested via service |
| `app/dev` | ❌ No direct command test | Tested via service |
| `app/build` | ❌ No direct command test | Tested via service |
| `app/env/show` | ❌ No test | |
| `app/function/*` | ❌ No direct command test | Tested via service |
| `app/webhook/*` | ❌ No test | |
| `app/version/*` | ❌ No test | |
| `app/import` | ❌ No test | |

**Only 4 of ~20+ app commands have dedicated test files.**

### 4.2 Theme Commands

| Command | Test | Coverage |
|---------|------|----------|
| `theme/check` | ✅ `commands/theme/check.test.ts` | ✅ |
| `theme/preview` | ✅ `commands/theme/preview.test.ts` | ✅ |
| `theme/info` | ✅ `commands/theme/info.test.ts` | ✅ |
| `theme/push` | ❌ No direct command test | Tested via service |
| `theme/pull` | ❌ No direct command test | Tested via service |
| `theme/dev` | ❌ No direct command test | Tested via service |
| `theme/delete` | ❌ No direct command test | Tested via service |
| `theme/package` | ❌ No direct command test | Tested via service |
| `theme/list` | ❌ No direct command test | Tested via service |
| `theme/publish` | ❌ No direct command test | Tested via service |
| `theme/open` | ❌ No direct command test | Tested via service |
| `theme/console` | ❌ No direct command test | Tested via service |
| `theme/rename` | ❌ No direct command test | Tested via service |
| `theme/duplicate` | ❌ No direct command test | Tested via service |

**Only 3 of ~14+ theme commands have dedicated test files.**

---

## 5. End-to-End Scenarios

### Location

`packages/e2e/`

### Framework

- **Runner**: Mocha
- **Assertions**: Chai
- **Setup**: Custom scripts in `package.json`

### Test Scope

- Focused on **app lifecycle** scenarios (create → deploy → update)
- Environment-aware (staging/production Shopify instances)
- Requires valid API credentials/tokens

### Assessment

The E2E suite is **minimal**. It covers happy-path app Lifecycle but does not exercise:
- Theme workflows
- Extension-specific flows (checkout, subscription UI, etc.)
- Error/misconfiguration scenarios
- Cross-version compatibility

---

## 6. Test Utilities

| Package/Location | Description |
|-----------------|-------------|
| `@shopify/app/models/app/app.test-data.ts` | Centralized test data factory — `testApp()`, `testAppWithConfig()`, `testOrganizationApp()`, `testPartnersUserSession()`, extension fixtures, web fixtures |
| `@shopify/ui-extensions-test-utils` | UI extension test helpers (React testing, mock host) |
| `@shopify/cli-kit/testing/*` | Testing utilities shipped with cli-kit |
| `packages/e2e/` | Custom E2E setup scripts |

### `app.test-data.ts` Factory Functions

| Function | Purpose |
|----------|---------|
| `testApp()` | Creates minimal `AppInterface` |
| `testAppWithConfig()` | App with full TOML configuration |
| `testOrganizationApp()` | Organization-level app fixture |
| `testPartnersUserSession()` | Mock partners session |
| Various extension factories | UI extension, theme extension, function, flow fixtures |

---

## 7. Coverage Gaps

### Critical Gaps (Untested Public API Surfaces)

| Surface | Risk | Notes |
|---------|------|-------|
| Storefront Renderer API | High | No tests at all — affects hydrogen/storefront rendering |
| Function Runner API | High | No API-level tests — affects function execution |
| App Management API | Medium | No tests — affects app management operations |
| Business Platform APIs | Medium | No tests — affects org/destination queries |
| All 35 graphql queries | Medium | Each query is untested — integration tests catch usage but individual query logic is unverified |

### Significant Gaps (Low Coverage)

| Area | Risk | Notes |
|------|------|-------|
| Command-level tests (app) | Medium | Only 4/20+ commands tested — most coverage is via services |
| Command-level tests (theme) | Medium | Only 3/14+ commands tested — most coverage is via services |
| E2E scenarios | Medium | Only app lifecycle — no theme, extensions, or edge cases |
| `partnersRequest()`/`partnersRequestDoc()` | Medium | Only tested via mock in PartnersClient — the actual HTTP/graphql call path is untested |

### Moderate Gaps (Partial Coverage)

| Area | Notes |
|------|-------|
| Theme API (`@shopify/cli-kit/node/api/theme`) | Could not find direct tests for the theme API client itself |
| Environment prompts | No dedicated tests for the prompt/unprompt flow |
| Integration between services | Tests mock heavily — few cross-service integration tests |

### Test Quality Observations

- **Mock-heavy**: Most tests mock the API layer, testing business logic in isolation — this is reasonable but means the API integration path is untested
- **No snapshot testing**: The test suite does not use snapshot/approval testing for command output
- **No property-based testing**: No fuzzing or generative testing
- **Coverage reports**: No evidence of tracked coverage metrics (istanbul, c8, etc.)

---

## Summary

```
┌─────────────────────────────────────────────────────────────┐
│                    Test Coverage Heatmap                      │
├─────────────────────────────────────────────────────────────┤
│  cli-kit helpers           ████████████████████░░  ~90%      │
│  Theme services            ████████████████████░░  ~90%      │
│  App services (deploy,     ██████████████████░░░░  ~80%      │
│    dev, build, function)                                     │
│  App commands              ████░░░░░░░░░░░░░░░░░░  ~20%      │
│  Theme commands            ███░░░░░░░░░░░░░░░░░░░  ~15%      │
│  API surfaces              ██░░░░░░░░░░░░░░░░░░░░  ~10%      │
│  GraphQL queries           ░░░░░░░░░░░░░░░░░░░░░░  ~0%       │
│  E2E scenarios             █░░░░░░░░░░░░░░░░░░░░░  ~5%       │
└─────────────────────────────────────────────────────────────┘
```

**Strengths**: cli-kit public helpers, theme services, app build/dev/deploy services.
**Weaknesses**: API surface integration, command-level testing, graphql query validation.
**Root cause**: Tests validate business logic through mocked API boundaries but rarely validate the API boundary itself. Commands are thin wrappers around services — the pattern was to test services, but command orchestration was left uncovered.
