# CLI-MAP: Shopify CLI Architecture

## 1. Entry Point

**File:** `packages/cli/bin/run.js` (production) / `packages/cli/bin/dev.js` (development)

Both files do the same thing: enable the Node.js compile cache, remove warning listeners, then dynamically import `../dist/bootstrap.js` and call its default export: `runCLI({development: boolean})`. The only difference is the `development` flag.

**Export called:** `runShopifyCLI({development})` — the default export from `packages/cli/src/bootstrap.ts`.

---

## 2. Build Pipeline

### Source Language
TypeScript (compiled to ESM JavaScript).

### Build toolchain
- **TypeScript Compiler** (`tsc`) via Nx targets for type-checking and declaration files
- **esbuild** (v0.28.1) for bundling into distributable artifacts (single-file bundles per package)
- **oclif** (v4.23.0) for CLI framework, manifests, and README generation
- **Nx** (v22.7.5) as the monorepo orchestrator

### Build chain (per package)
```
src/*.ts  --tsc-->  dist/*.js + dist/*.d.ts  --esbuild-->  bundled executable
```

### Key configuration files
| File | Purpose |
|------|---------|
| `tsconfig.base.json` | Base TypeScript settings (ES2020, NodeNext modules) |
| `configurations/tsconfig.json` | Shared TS config for all packages (composite, path aliases) |
| `packages/<pkg>/tsconfig.json` | Per-package TS config, extends base, references deps |
| `packages/<pkg>/vite.config.ts` | Delegates to `configurations/vite.config.ts` |
| `configurations/vite.config.ts` | Shared Vite config factory; sets up test aliases for `@shopify/cli-kit`, `@shopify/theme`, `@shopify/organizations`, `@shopify/store` pointing to `src/` |
| `nx.json` | Nx workspace config, target dependencies, caching |
| `packages/<pkg>/project.json` | Per-package Nx targets (build, bundle, lint, type-check, refresh-manifests) |

### Build targets (from `project.json`)
- **build:** `tsc -b ./tsconfig.build.json` — type checks and emits declarations
- **bundle:** esbuild-driven (custom `bin/bundle` script in packages/cli) — produces the single-file distributable
- **Bundle depends on:** build → (^build of dependencies)

### Nx task pipeline
```
clean -> ^clean
build -> ^build (build dependencies first)
bundle -> build -> cli-kit:generate-version, app:build, theme:build
refresh-manifests -> build, refresh-readme
type-check -> ^build
```

### GraphQL code generation
The `graphql-codegen` step reads `.graphql` files (raw queries/mutations) and generates typed TypeScript documents in `generated/` directories using `@graphql-codegen/*` packages.

---

## 3. Monorepo Structure

### Workspace configuration
**File:** `pnpm-workspace.yaml`
```
packages:
  - packages/*
  - workspace
```

### Packages

| Package | npm name | Responsibility | Dependencies |
|---------|----------|---------------|--------------|
| `packages/cli` | `@shopify/cli` | Main CLI binary; registers all commands, commands are composed from other packages | `@shopify/app`, `@shopify/theme`, `@shopify/cli-kit`, `@shopify/plugin-cloudflare`, `@shopify/plugin-did-you-mean`, `@shopify/store`, `@shopify/cli-hydrogen` |
| `packages/cli-kit` | `@shopify/cli-kit` | Core SDK: CLI launcher, API clients (REST/GraphQL), session/auth, output/UI rendering, file system, error handling, analytics | (none — foundation layer) |
| `packages/app` | `@shopify/app` | App development commands: `dev`, `deploy`, `build`, `init`, `generate extension`, `env`, `config`, `logs`, `function`, `webhook trigger`, bulk operations, versions | `@shopify/cli-kit` |
| `packages/theme` | `@shopify/theme` | Theme commands: `push`, `pull`, `dev`, `delete`, `list`, `info`, `open`, `share`, `language-server`, `check` | `@shopify/cli-kit` |
| `packages/create-app` | `@shopify/create-app` | Standalone `create-app` CLI; auto-injects the `init` command | `@shopify/app`, `@shopify/cli-kit` |
| `packages/store` | `@shopify/store` | Store operations | `@shopify/cli-kit` |
| `packages/organizations` | `@shopify/organizations` | Organization-level operations | `@shopify/cli-kit` |
| `packages/plugin-cloudflare` | `@shopify/plugin-cloudflare` | Cloudflare tunnel provider plugin | `@shopify/cli-kit` |
| `packages/plugin-did-you-mean` | `@shopify/plugin-did-you-mean` | Command autocorrection ("did you mean?") | `@shopify/cli-kit` |
| `packages/e2e` | (private) | End-to-end Playwright tests | All packages |
| `packages/eslint-plugin-cli` | `@shopify/eslint-plugin-cli` | ESLint rules for the CLI codebase | (dev only) |
| `packages/ui-extensions-dev-console` | (private) | React dev console for UI extensions | `@shopify/ui-extensions-server-kit` |
| `packages/ui-extensions-server-kit` | (private) | Server kit for UI extension development | (standalone) |
| `packages/ui-extensions-test-utils` | (private) | Test utilities for UI extensions | (standalone) |

### Dependency graph
```
cli-kit <-- app <-- create-app
cli-kit <-- theme <-- cli
cli-kit <-- store <-- cli
cli-kit <-- organizations
cli-kit <-- plugin-cloudflare <-- cli
cli-kit <-- plugin-did-you-mean <-- cli
app <-- cli (implicit)
theme <-- cli (implicit)
```

### Package.json structure pattern
- `"type": "module"` — all packages are ESM
- `"exports"` — maps package root to `./dist/index.js` and subpath exports to `./dist/public/*.js`
- `"bin"` — `packages/cli` has `shopify: bin/run.js`
- `oclif` config in package.json defines commands, hooks, topics, and the manifest strategy

---

## 4. Component Inventory

### 4.1 CLI Framework Layer (`packages/cli-kit/src/public/node/`)

#### `cli.ts`
- **Path:** `packages/cli-kit/src/public/node/cli.ts`
- **Responsibility:** Sets up environment variables, Node.js version check, forces no-color, then delegates to `cli-launcher.ts`
- **Exports:**
  - `runCLI(options)` — main entry, calls launchCLI
  - `runCreateCLI(options)` — wraps runCLI, auto-injects "init" for `create-*` CLIs
  - `globalFlags` — shared `--no-color` and `--verbose` flags
  - `jsonFlag` — shared `--json` / `-j` flag
  - `portFlag(options)` — returns a validated `--port` integer flag (1-65535)
  - `clearCache()` — clears Confstore cache

#### `cli-launcher.ts`
- **Path:** `packages/cli-kit/src/public/node/cli-launcher.ts`
- **Responsibility:** Loads the oclif `ShopifyConfig`, runs the CLI, handles top-level errors
- **Exports:**
  - `launchCLI(options)` — creates `ShopifyConfig`, calls `Config.load()`, calls `Oclif.run()` and `flush()`

#### `base-command.ts`
- **Path:** `packages/cli-kit/src/public/node/base-command.ts`
- **Responsibility:** Abstract base class for all CLI commands; handles init (analytics, bugsnag), parse (environments), error catching, npm flag warnings
- **Exports:**
  - `default` — `BaseCommand` abstract class
  - `ArgOutput` — type alias for oclif parsed args
  - `FlagOutput` — type alias for oclif parsed flags
  - `addFromParsedFlags(flags)` — adds path/verbose flags to public metadata
  - `noDefaultsOptions(options)` — strips defaults from flag definitions for environment detection

#### `custom-oclif-loader.ts`
- **Path:** `packages/cli-kit/src/public/node/custom-oclif-loader.ts`
- **Responsibility:** Custom oclif Config subclass that supports lazy command loading from a manifest; avoids loading all commands at startup
- **Exports:**
  - `ShopifyConfig` — Config class with `setLazyCommandLoader(loader)` method
  - `LazyCommandLoader` — type

---

### 4.2 API Layer (`packages/cli-kit/src/public/node/api/`)

#### `graphql.ts`
- **Path:** `packages/cli-kit/src/public/node/api/graphql.ts`
- **Responsibility:** Core GraphQL client; handles HTTP client creation, rate-limit waiting, caching, token refresh on 401, request retry, telemetry timing
- **Exports:**
  - `graphqlRequest<T>(options)` — executes a raw GraphQL query string
  - `graphqlRequestDoc<TResult, TVariables>(options)` — executes a typed document node
  - `GraphQLResponse<T>` — type
  - `GraphQLVariables` — type alias `Record<string, any>`
  - `CacheOptions` — interface (`cacheTTL: TimeInterval`, `cacheExtraKey?`, `cacheStore?`)
  - `UnauthorizedHandler` — interface (`type: 'token_refresh'`, `handler: () => Promise<{token?: string}>`)
  - `GraphQLResponseOptions` — interface (`handleErrors?`, `onResponse?`)
  - `GraphQLRequestOptions` — interface
  - `GraphQLRequestDocOptions` — interface
  - `CacheOptions` — interface

#### `partners.ts`
- **Path:** `packages/cli-kit/src/public/node/api/partners.ts`
- **Responsibility:** Partners API client; rate-limited (Bottleneck: 150ms minTime, 10 concurrent). Registers deprecation handler. Builds URL as `https://{partnersFqdn}/api/cli/graphql`
- **Exports:**
  - `partnersRequest<T>(query, token, variables?, cacheOptions?, preferredBehaviour?, unauthorizedHandler?)` — raw query
  - `partnersRequestDoc<TResult, TVariables>(query, token, variables?, preferredBehaviour?, unauthorizedHandler?)` — typed document
  - `generateFetchAppLogUrl(cursor?, filters?)` — builds polling URL for app logs
  - `handleDeprecations(response)` — extracts deprecation dates from response extensions

#### `admin.ts`
- **Path:** `packages/cli-kit/src/public/node/api/admin.ts`
- **Responsibility:** Shopify Admin API client (GraphQL + REST). Auto-discovers latest supported API version. Handles theme access sessions with different headers/URLs.
- **Exports:**
  - `adminRequest<T>(query, session, variables?)` — GraphQL query
  - `adminRequestDoc<TResult, TVariables>(options)` — typed document GraphQL
  - `supportedApiVersions(session, preferredBehaviour?)` — returns string[]
  - `fetchApiVersions(session, preferredBehaviour?)` — returns raw ApiVersion[]
  - `adminUrl(store, version, session?)` — builds Admin API URL
  - `restRequest(method, path, session, requestBody?, searchParams?, apiVersion?)` — REST HTTP request
  - `RestResponse` — interface (`json`, `status`, `headers`)
  - `AdminRequestOptions` — interface

#### `app-dev.ts`
- **Path:** `packages/cli-kit/src/public/node/api/app-dev.ts`
- **Responsibility:** App Dev API client; rate-limited. URL: `https://{appDevFqdn}/app_dev/unstable/graphql.json`
- **Exports:**
  - `appDevRequestDoc<TResult, TVariables>(options)` — typed document
  - `AppDevRequestOptions` — interface

#### `app-management.ts`
- **Path:** `packages/cli-kit/src/public/node/api/app-management.ts`
- **Responsibility:** App Management API client; rate-limited. URL: `https://{appManagementFqdn}/app_management/unstable/graphql.json`
- **Exports:**
  - `appManagementRequestDoc<TResult, TVariables>(options)` — typed document
  - `appManagementHeaders(token)` — builds headers
  - `appManagementAppLogsUrl(orgId, cursor?, filters?)` — builds app logs polling URL
  - `handleDeprecations(response)` — extracts deprecation dates
  - `AppManagementRequestOptions` — interface
  - `RequestOptions` — interface

#### `business-platform.ts`
- **Path:** `packages/cli-kit/src/public/node/api/business-platform.ts`
- **Responsibility:** Business Platform API client. Two sub-APIs: Destinations (`https://{fqdn}/destinations/api/2020-07/graphql`) and Organizations (`https://{fqdn}/organizations/api/unstable/organization/{orgId}/graphql`)
- **Exports:**
  - `businessPlatformRequest<T>(query, token, variables?, cacheOptions?)` — raw query (Destinations)
  - `businessPlatformRequestDoc<TResult, TVariables>(options)` — typed document (Destinations)
  - `businessPlatformOrganizationsRequest<T>(options)` — raw query (Organizations)
  - `businessPlatformOrganizationsRequestDoc<TResult, TVariables>(options)` — typed document (Organizations)
  - `BusinessPlatformRequestOptions` — interface
  - `BusinessPlatformOrganizationsRequestOptions` — interface

#### `webhooks.ts`
- **Path:** `packages/cli-kit/src/public/node/api/webhooks.ts`
- **Responsibility:** Webhooks GraphQL API; rate-limited. URL: `https://{appManagementFqdn}/webhooks/unstable/organizations/{orgId}/graphql.json`
- **Exports:**
  - `webhooksRequestDoc<TResult, TVariables>(options)` — typed document
  - `WebhooksRequestOptions` — interface

#### `functions.ts`
- **Path:** `packages/cli-kit/src/public/node/api/functions.ts`
- **Responsibility:** App Management Functions API; rate-limited. URL: `https://{appManagementFqdn}/functions/unstable/organizations/{orgId}/{appId}/graphql`
- **Exports:**
  - `functionsRequestDoc<TResult, TVariables>(options)` — typed document
  - `FunctionsRequestOptions` — interface

#### `utilities.ts`
- **Path:** `packages/cli-kit/src/public/node/api/utilities.ts`
- **Responsibility:** Shared API URL utilities
- **Exports:**
  - `addCursorAndFiltersToAppLogsUrl(baseUrl, cursor?, filters?)` — appends `cursor`, `status`, `source` query params

---

### 4.3 HTTP Layer (`packages/cli-kit/src/public/node/http.ts`)

#### `http.ts`
- **Path:** `packages/cli-kit/src/public/node/http.ts`
- **Responsibility:** HTTP fetch abstraction with retry, TLS agent, abort signals, request mode presets, file download
- **Exports:**
  - `fetch(url, init?, preferredBehaviour?)` — generic fetch (retries disabled)
  - `shopifyFetch(url, init?, preferredBehaviour?)` — Shopify-specific fetch (retries enabled)
  - `downloadFile(url, to)` — download to local path
  - `formData()` — creates FormData instance
  - `requestMode(preset?, env?)` — resolves RequestBehaviour from preset string or object
  - `abortSignalFromRequestBehaviour(behaviour)` — creates AbortSignal
  - `RequestBehaviour` — type
  - `RequestModeInput` — type

---

### 4.4 Session and Auth (`packages/cli-kit/src/public/node/session.ts`)

#### `session.ts`
- **Path:** `packages/cli-kit/src/public/node/session.ts`
- **Responsibility:** Authentication orchestration; ensures valid sessions for all API types (Partners, Admin, App Management, Business Platform, Storefront, Themes)
- **Exports:**
  - `AdminSession` — interface (`token`, `storeFqdn`)
  - `Session` — interface (`token`, `businessPlatformToken`, `accountInfo`, `userId`)
  - `AccountInfo` — union type
  - `ensureAuthenticatedUser(env?, options?)` — no-scope auth
  - `ensureAuthenticatedPartners(scopes?, env?, options?)`
  - `ensureAuthenticatedAppManagementAndBusinessPlatform(options?, appManagementScopes?, businessPlatformScopes?, env?)`
  - `ensureAuthenticatedStorefront(scopes?, password?, options?)`
  - `ensureAuthenticatedAdmin(store, scopes?, options?)`
  - `ensureAuthenticatedThemes(store, password, scopes?, options?)`
  - `ensureAuthenticatedBusinessPlatform(scopes?, options?)`
  - `ensureAuthenticatedAdminAsApp(storeFqdn, clientId, clientSecret)` — OAuth client_credentials grant
  - `logout()` — removes stored sessions
  - `setLastSeenUserId(userId)` — records user ID
  - `isUserAccount(account)` — type guard
  - `isServiceAccount(account)` — type guard

---

### 4.5 Error Handling (`packages/cli-kit/src/public/node/error-handler.ts`)

#### `error-handler.ts`
- **Path:** `packages/cli-kit/src/public/node/error-handler.ts`
- **Responsibility:** Top-level error handling; maps errors, sends analytics, reports to Bugsnag in production
- **Exports:**
  - `errorHandler(error, config?)` — main error handler
  - `sendErrorToBugsnag(error, exitMode)` — sends error to Bugsnag
  - `cleanStackFrameFilePath({...})` — normalizes stack file paths
  - `registerCleanBugsnagErrorsFromWithinPlugins(config)` — adds Bugsnag error listener
  - `addBugsnagMetadata(event, config)` — attaches metadata to Bugsnag events

---

### 4.6 UI Layer (`packages/cli-kit/src/public/node/ui.tsx` and subdirs)

- **Path:** `packages/cli-kit/src/private/node/ui/`
- **Responsibility:** Ink-based React terminal UI components
- **Components (all under `private/node/ui/components/`):**
  - `Alert` — alert banners
  - `AutocompletePrompt` — autocomplete input
  - `Banner` — info/warning/error banners
  - `Command` — command display
  - `ConcurrentOutput` — parallel output streams
  - `DangerousConfirmationPrompt` — destructive action confirmation
  - `FatalError` — fatal error screen
  - `FilePath` — file path display
  - `Link` — clickable links
  - `List` — item lists
  - `LoadingBar` — progress bar
  - `Scrollbar` — scrollable container
  - `SelectInput` / `SelectPrompt` — selection menus
  - `SingleTask` / `Tasks` — task progress displays
  - `TextAnimation` — animated text
  - `TextInput` / `TextPrompt` — text input
  - `TokenizedText` — rich text tokens
  - `UserInput` — user input display
  - **Subdirs:** `Prompts/` (InfoMessage, InfoTable, PromptLayout), `Table/` (Column, Row, Table)

#### Rendering entry: `ui.tsx`
- **Path:** `packages/cli-kit/src/public/node/ui.tsx`
- **Exports:** `renderOnce`, `renderInfo`, `renderSuccess`, `renderWarning`, `renderError`, `renderFatalError`, `renderConfirmationPrompt`, `renderTextPrompt`, `renderSelectPrompt`, `renderTasks`, `renderConcurrent`, etc.

---

### 4.7 App Package (`packages/app/src/cli/`)

#### `commands/app/`
| Command | File | Responsibility |
|---------|------|----------------|
| `dev` | `commands/app/dev.ts` | Run app locally with hot-reload |
| `deploy` | `commands/app/deploy.ts` | Upload and release app version |
| `build` | `commands/app/build.ts` | Build app extensions |
| `init` | `commands/app/init.ts` | Scaffold a new app |
| `info` | `commands/app/info.ts` | Show app configuration info |
| `logs` | `commands/app/logs.ts` | Stream app logs |
| `release` | `commands/app/release.ts` | Release a version |
| `execute` | `commands/app/execute.ts` | Execute bulk operation |
| `import-extensions` | `commands/app/import-extensions.ts` | Import dashboard extensions |
| `import-custom-data-definitions` | `commands/app/import-custom-data-definitions.ts` | Import custom data definitions |
| `config/link` | `commands/app/config/link.ts` | Link local app to remote |
| `config/pull` | `commands/app/config/pull.ts` | Pull remote config |
| `config/use` | `commands/app/config/use.ts` | Switch app config |
| `config/validate` | `commands/app/config/validate.ts` | Validate app config |
| `env/pull` | `commands/app/env/pull.ts` | Pull environment variables |
| `env/show` | `commands/app/env/show.ts` | Show environment variables |
| `function/build` | `commands/app/function/build.ts` | Build a function |
| `function/run` | `commands/app/function/run.ts` | Run a function locally |
| `function/replay` | `commands/app/function/replay.ts` | Replay function events |
| `function/schema` | `commands/app/function/schema.ts` | Generate function schema |
| `function/typegen` | `commands/app/function/typegen.ts` | Generate types |
| `function/info` | `commands/app/function/info.ts` | Show function info |
| `generate/extension` | `commands/app/generate/extension.ts` | Scaffold an extension |
| `versions/list` | `commands/app/versions/list.ts` | List app versions |
| `webhook/trigger` | `commands/app/webhook/trigger.ts` | Send a sample webhook |
| `bulk/cancel` | `commands/app/bulk/cancel.ts` | Cancel a bulk operation |
| `bulk/execute` | `commands/app/bulk/execute.ts` | Run a bulk operation query/mutation |
| `bulk/status` | `commands/app/bulk/status.ts` | Check bulk operation status |
| `demo/watcher` | `commands/app/demo/watcher.ts` | Demo file watcher |
| `dev/clean` | `commands/app/dev/clean.ts` | Clean dev data |

#### `developer-platform-client.ts` (abstraction)
- **Path:** `packages/app/src/cli/utilities/developer-platform-client.ts`
- **Responsibility:** Abstraction over Partners API and App Management API; presents a unified interface
- **Interface:** `DeveloperPlatformClient` — ~50 methods covering orgs, apps, extensions, versions, deploys, webhooks, dev sessions, logs
- **Implementations:**
  - `partners-client.ts` — calls `partnersRequest`/`partnersRequestDoc` from cli-kit
  - `app-management-client.ts` — calls `appManagementRequestDoc`, `businessPlatformOrganizationsRequestDoc`, `webhooksRequestDoc`, `functionsRequestDoc`, `appDevRequestDoc` from cli-kit

#### `models/app/app.ts`
- **Path:** `packages/app/src/cli/models/app/app.ts`
- **Responsibility:** App model — representation of a Shopify app project with its extensions, configuration, and identifiers

#### `models/extensions/`
- **Path:** `packages/app/src/cli/models/extensions/`
- **Responsibility:** All built-in extension type specifications (UI Extension, Theme Extension, Function, Flow, Webhook, etc.)

---

### 4.8 CLI Commands (`packages/cli/src/cli/commands/`)

| Command | File | Responsibility |
|---------|------|----------------|
| `version` | `cli/commands/version.ts` | Show CLI version |
| `upgrade` | `cli/commands/upgrade.ts` | Upgrade CLI |
| `search` | `cli/commands/search.ts` | Open search on shopify.dev |
| `help` | `cli/commands/help.ts` | Custom help display |
| `auth/login` | `cli/commands/auth/login.ts` | Authenticate |
| `auth/logout` | `cli/commands/auth/logout.ts` | Logout |
| `cache/clear` | `cli/commands/cache/clear.ts` | Clear CLI cache |
| `config/autoupgrade/on` | `cli/commands/config/autoupgrade/on.ts` | Enable auto-upgrade |
| `config/autoupgrade/off` | `cli/commands/config/autoupgrade/off.ts` | Disable auto-upgrade |
| `config/autoupgrade/status` | `cli/commands/config/autoupgrade/status.ts` | Check auto-upgrade status |
| `debug/command-flags` | `cli/commands/debug/command-flags.ts` | Debugging command flags |
| `docs/generate` | `cli/commands/docs/generate.ts` | Generate API docs |
| `doctor-release` | `cli/commands/doctor-release/doctor-release.ts` | Pre-release doctor checks |
| `kitchen-sink/*` | `cli/commands/kitchen-sink/` | UI component demos (hidden) |
| `notifications/generate` | `cli/commands/notifications/generate.ts` | Generate notifications |
| `notifications/list` | `cli/commands/notifications/list.ts` | List notifications |

---

### 4.9 Hooks (lifecycle interceptors)

| Hook | File | Responsibility |
|------|------|----------------|
| `init` | `hooks/app-init.ts` | App-specific initialization |
| `init` | `hooks/hydrogen-init.ts` | Hydrogen-specific initialization |
| `prerun` | `hooks/prerun.ts` | Executed before every command |
| `postrun` | `hooks/postrun.ts` | Executed after every command |
| `command_not_found` | `hooks/did-you-mean.ts` | Autocorrection suggestions |
| `tunnel_start` | `hooks/tunnel-start.ts` | Start tunnel |
| `tunnel_provider` | `hooks/tunnel-provider.ts` | Tunnel provider resolution |
| `update` | `hooks/plugin-plugins.ts` | Plugin updates |
| `sensitive_command_metadata` | `hooks/sensitive-metadata.ts` | Collect sensitive metadata |
| `public_command_metadata` | `hooks/public-metadata.ts` | Collect public metadata |

---

### 4.10 Theme Package (`packages/theme/`)

- **Commands:** `push`, `pull`, `dev`, `delete`, `list`, `info`, `open`, `share`, `language-server`, `check`
- **Relies on:** `packages/cli-kit/src/public/node/themes/api.ts` for all Admin API interactions

---

### 4.11 Store Package (`packages/store/`)
- **Responsibility:** Commands for working directly with Shopify stores
- **Relies on:** `packages/cli-kit`

### 4.12 Organizations Package (`packages/organizations/`)
- **Responsibility:** Organization-level operations
- **Relies on:** `packages/cli-kit`

---

## 5. Helper Functions

### 5.1 Common Utilities (`packages/cli-kit/src/public/common/`)

| Function | File | Purpose | Signature | Called by |
|----------|------|---------|-----------|-----------|
| `filter(iterable, predicate)` | `array.ts` | Filter with proper typing | `<T>(iterable: Iterable<T>, predicate: (item: T) => boolean) => T[]` | Various |
| `getArrayRejectingUndefined(arr)` | `array.ts` | Remove undefined values | `<T>(arr: (T \| undefined)[]) => T[]` | services/deploy.ts |
| `groupBy(array, keyFn)` | `collection.ts` | Group array by key | `<T, K>(array: T[], keyFn: (item: T) => K) => Map<K, T[]>` | Various |
| `isUnitInterval(value)` | `lang.ts` | Check if value is 0-1 | `(value: unknown) => value is number` | Various |
| `nonRandomUUID(value)` | `string.ts` | Deterministic UUID from string | `(value: string) => string` | graphql.ts (cache keys), session.ts |
| `underscore(str)` | `string.ts` | Convert camelCase to snake_case | `(str: string) => string` | base-command.ts |
| `tryParseJson(str)` | `json.ts` | Parse JSON safely | `(str: string) => JsonMap \| undefined` | Various |

### 5.2 Node Utilities (`packages/cli-kit/src/public/node/`)

| Function | File | Purpose | Signature | Called by |
|----------|------|---------|-----------|-----------|
| `hashString(str)` | `crypto.ts` | SHA-256 hex hash | `(str: string) => string` | base-command.ts (flags path hash) |
| `isDevelopment()` | `context/local.ts` | Check if running in development | `() => boolean` | base-command.ts, cli-launcher.ts |
| `isTruthy(value)` | `context/utilities.ts` | Check if env var is truthy | `(value: string \| undefined) => boolean` | cli.ts, base-command.ts |
| `sanitizeURL(url)` | `private/node/api/urls.ts` | Remove query params for logging | `(url: string) => string` | http.ts |
| `sanitizedHeadersOutput(headers)` | `private/node/api/headers.ts` | Filter sensitive headers for logging | `(headers: Record<string, string>) => string` | http.ts |
| `buildHeaders(token)` | `private/node/api/headers.ts` | Build standard auth headers | `(token?: string) => Record<string, string>` | All API clients |
| `httpsAgent()` | `private/node/api/headers.ts` | Build TLS agent | `() => Promise<Agent>` | http.ts, graphql.ts |
| `retryAwareRequest(options, errorHandler?)` | `private/node/api.ts` | Retry-aware HTTP request | `(options, errorHandler?) => Promise<Response>` | graphql.ts |
| `isNetworkError(error)` | `private/node/api.ts` | Check if error is network-level | `(error) => boolean` | admin.ts |
| `errorMapper(error)` | `error.ts` | Map errors for user display | `(error) => Promise<MappedError>` | error-handler.ts |
| `shouldReportErrorAsUnexpected(error)` | `error.ts` | Check if error should be reported as bug | `(error) => boolean` | error-handler.ts |
| `handler(mappedError)` | `error.ts` | Render error to user | `(mappedError) => void` | error-handler.ts |
| `simpleRequestWithDebugLog(options)` | `private/node/api.ts` | Single request with retry + debug | `(options) => Promise<Response>` | http.ts |
| `sleep(seconds)` | `system.ts` | Promise-based sleep | `(seconds: number) => Promise<void>` | graphql.ts |
| `addPublicMetadata(factory)` | `metadata.ts` | Add to public metadata | `(factory: () => JsonMap \| Promise<JsonMap>) => Promise<void>` | base-command.ts, graphql.ts |
| `runWithTimer(key)` | `metadata.ts` | Time a function, store in metadata | `(key: string) => <T>(fn: () => Promise<T>) => Promise<T>` | http.ts, graphql.ts |
| `nonRandomUUID(str)` | `crypto.ts` | SHA-256 → hex → UUID format | `(str: string) => string` | graphql.ts (caching), session.ts |
| `cacheRetrieveOrRepopulate(key, fetcher, ttlMs, store?)` | `private/node/conf-store.ts` | Cache-aside with TTL | `(key, fetcher, ttlMs, store?) => Promise<string>` | graphql.ts |
| `initializeBugsnag()` | `error-handler.ts` | Start Bugsnag if not started | `() => void` | error-handler.ts |
| `CLI_KIT_VERSION` | `common/version.ts` | Package version constant | `string` | graphql.ts, error-handler.ts |
| `formatPackageManagerCommand(pm, cmd)` | `output.ts` | Format CLI command string | `(pm: PackageManager, cmd: string) => string` | partners.ts |
| `outputContent` / `outputToken` / `outputDebug` / `outputInfo` / `outputResult` | `output.ts` | Structured terminal output | Various | Throughout |
| `partnersFqdn()` | `context/fqdn.ts` | Resolve Partners API FQDN | `() => Promise<string>` | partners.ts |
| `appManagementFqdn()` | `context/fqdn.ts` | Resolve App Management API FQDN | `() => Promise<string>` | app-management.ts |
| `businessPlatformFqdn()` | `context/fqdn.ts` | Resolve Business Platform FQDN | `() => Promise<string>` | business-platform.ts |
| `appDevFqdn(shopFqdn)` | `context/fqdn.ts` | Resolve App Dev API FQDN | `(shopFqdn) => Promise<string>` | app-dev.ts |
| `normalizeStoreFqdn(fqdn)` | `context/fqdn.ts` | Normalize store domain | `(fqdn) => string` | app-dev.ts |
| `serviceEnvironment()` | `private/node/context/service.ts` | Get env (`local` / `production`) | `() => string` | admin.ts |
| `blockPartnersAccess()` | `environment.ts` | Check SHOPIFY_CLI_NEVER_USE_PARTNERS_API | `() => boolean` | partners.ts, developer-platform-client.ts |
| `firstPartyDev()` | `context/local.ts` | Check if connected as first-party dev | `() => boolean` | developer-platform-client.ts |
| `getAppAutomationToken()` | `environment.ts` | Read SHOPIFY_APP_AUTOMATION_TOKEN | `() => string \| undefined` | session.ts |
| `skipNetworkLevelRetry(env)` | `environment.ts` | Check SHOPIFY_CLI_SKIP_NETWORK_LEVEL_RETRY | `(env?) => boolean` | http.ts |
| `maxRequestTimeForNetworkCallsMs(env)` | `environment.ts` | Max request timeout | `(env?) => number` | http.ts |
| `themeGid(id)` | `themes/api.ts` | Compose theme GID | `(id: number) => string` | themes/api.ts |
| `parseGid(gid)` | `themes/utils.ts` | Extract ID from GID | `(gid: string) => number` | themes/api.ts |
| `composeThemeGid(id)` | `themes/utils.ts` | Compose OnlineStoreTheme GID | `(id: number) => string` | themes/api.ts |
| `buildTheme({id, name, role, processing})` | `themes/factories.ts` | Create Theme domain object | `(input) => Theme \| undefined` | themes/api.ts |
| `parseThemeFileContent(body)` | `themes/api.ts` | Decode theme file body | `(body) => Promise<{value?, attachment?}>` | themes/api.ts |
| `addCursorAndFiltersToAppLogsUrl(baseUrl, cursor?, filters?)` | `api/utilities.ts` | Build app logs poll URL | `(url, cursor?, filters?) => string` | partners.ts, app-management.ts |
| `isThemeAccessSession(session)` | `private/node/api/rest.ts` | Check if token is shptka_ | `(session) => boolean` | admin.ts, rest.ts |
| `setCurrentCommandId(id)` | `global-context.ts` | Track current command ID | `(id: string) => void` | base-command.ts |

---

## 6. Control Flow

### 6.1 Typical CLI invocation (e.g., `shopify app dev`)

```
1. bin/run.js (or bin/dev.js)
   ├── Enable Node compile cache
   ├── Remove warning listeners
   └── Dynamic import: dist/bootstrap.js → runShopifyCLI({development: false})

2. bootstrap.ts
   ├── createGlobalProxyAgent() (support SHOPIFY_http_proxy etc.)
   ├── setupEnvironmentVariables() (DEBUG=* for --verbose, SHOPIFY_CLI_ENV)
   ├── forceNoColor() (check --no-color, --json, NO_COLOR, TERM=dumb)
   ├── exitIfOldNodeVersion() (exit if Node < 18)
   └── launchCLI() → cli-launcher.ts

3. cli-launcher.ts
   ├── new ShopifyConfig(root=fileURLToPath(moduleURL))
   ├── config.load() (reads oclif.manifest.json, registers plugins)
   ├── If lazyCommandLoader: config.setLazyCommandLoader(loadCommand)
   └── oclif.run(argv, config) + flush()

4. oclif core:
   ├── Parse argv → identify command ("app:dev")
   ├── Load command class from manifest (lazy via command-registry.ts)
   ├── Execute init hooks (app-init.ts, hydrogen-init.ts)
   ├── Execute prerun hook (prerun.ts — analytics, etc.)
   ├── Command.init()
   │   ├── setCurrentCommandId()
   │   ├── If not development: registerCleanBugsnagErrorsFromWithinPlugins()
   │   ├── removeDuplicatedPlugins() (warn/remove bundled plugins)
   │   ├── showNpmFlagWarning()
   │   ├── showNotificationsIfNeeded()
   │   └── super.init()
   ├── Command.parse() → parse flags, apply environment overrides
   │   ├── super.parse() (oclif)
   │   ├── If environments file exists: loadEnvironment → merge env flags
   │   └── addFromParsedFlags() → metadata tracking
   ├── Command.run() → actual command logic
   │   └── (e.g. services/dev.ts) →
   │       ├── ensureAuthenticatedPartners() / ensureAuthenticatedAdmin()
   │       ├── Fetch orgs, apps, stores via DeveloperPlatformClient
   │       │   ├── partnersRequestDoc() (GraphQL to Partners API)
   │       │   └── adminRequestDoc() (GraphQL to Admin API)
   │       ├── Build/compile extensions
   │       ├── Dev server, file watcher, tunnel
   │       └── UI rendering (Ink React components)
   ├── Execute postrun hook (postrun.ts — analytics flush)
   └── Report errors via errorHandler() → Bugsnag + analytics

5. On error at any point:
   └── BaseCommand.catch() → errorHandler()
       ├── If CancelExecution: show message
       ├── If AbortSilentError: do nothing
       └── Else: errorMapper() → handler() → renderFatalError()
           → reportAnalyticsEvent() (Monorail)
           → sendErrorToBugsnag() (production only)
```

### 6.2 API request flow (GraphQL example)

```
partnersRequestDoc(query, token, variables)
  → limiter.schedule() (Bottleneck rate limiter: 150ms minTime)
    → graphqlRequestDoc(options)
      → resolveRequestDocument(query) → get raw query string
      → createGraphQLClient(url, addedHeaders, token)
        → buildHeaders(token) (Authorization: Bearer, User-Agent, etc.)
        → new GraphQLClient(url, {agent: httpsAgent(), headers})
      → performGraphQLRequest()
        → runSingleRawGraphQLRequest()
          → client.rawRequest(query, variables)
            (if autoRateLimitRestore: sleep to restore rate limit)
        → retryAwareRequest() (auto-retry on network errors)
        → If 401 and tokenRefreshHandler: refresh token, retry
        → Run with timer (cmd_all_timing_network_ms)
        → If cacheOptions: cacheRetrieveOrRepopulate()
        → On response: logLastRequestIdFromResponse()
```

### 6.3 Create-app flow

```
bin/dev.js → runCreateApp(true)
  → create-app/src/index.ts
    → runCreateCLI()
      → runCLI({runInCreateMode: true})
        → addInitToArgvWhenRunningCreateCLI()
          (auto-injects "init" command for create-* CLIs)
        → launchCLI()
          → oclif.run(argv) (now includes "init")
            → init command from @shopify/app
```

### 6.4 Developer Platform Client dispatch

```
selectDeveloperPlatformClient({organization})
  → if org.source === BusinessPlatform → use AppManagementClient
  → else if firstPartyDev() && !blockPartnersAccess() → use PartnersClient
  → else → use AppManagementClient
```
