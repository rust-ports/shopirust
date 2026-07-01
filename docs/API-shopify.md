# API-shopify: Shopify API Calls Inventory

> **Note on URL construction:** All FQDNs are resolved at runtime via context/fqdn.ts, enabling environment-specific overrides (production vs. staging). The URLs below show the pattern with `{fqdn}` placeholders.

---

## API Client Architecture

The CLI communicates with Shopify through **6 distinct API surfaces**, each with its own client in `packages/cli-kit/src/public/node/api/`:

| API Surface | Client file | Base URL pattern | Rate limiter |
|-------------|-------------|------------------|--------------|
| **Partners API** | `partners.ts` | `https://{partnersFqdn}/api/cli/graphql` | Bottleneck: 150ms / 10 concurrent |
| **Admin API** (GraphQL) | `admin.ts` | `https://{store}/admin/api/{version}/graphql.json` | None |
| **Admin API** (REST) | `admin.ts` (via `restRequest`) | `https://{store}/admin/api/{version}{path}.json` | None |
| **App Management API** | `app-management.ts` | `https://{appManagementFqdn}/app_management/unstable/graphql` | Bottleneck: 150ms / 10 concurrent |
| **App Dev API** | `app-dev.ts` | `https://{appDevFqdn}/app_dev/unstable/graphql.json` | Bottleneck: 150ms / 10 concurrent |
| **Business Platform API** (Destinations) | `business-platform.ts` | `https://{businessPlatformFqdn}/destinations/api/2020-07/graphql` | None |
| **Business Platform API** (Organizations) | `business-platform.ts` | `https://{businessPlatformFqdn}/organizations/api/unstable/organization/{orgId}/graphql` | None |
| **Webhooks API** | `webhooks.ts` | `https://{appManagementFqdn}/webhooks/unstable/organizations/{orgId}/graphql.json` | Bottleneck: 150ms / 10 concurrent |
| **Functions API** | `functions.ts` | `https://{appManagementFqdn}/functions/unstable/organizations/{orgId}/{appId}/graphql` | Bottleneck: 150ms / 10 concurrent |

All GraphQL clients share the core engine in `packages/cli-kit/src/public/node/api/graphql.ts`, which provides:
- Automatic retry with backoff
- Rate-limit waiting (GraphQL cost-based throttleStatus)
- Caching via `conf-store.ts` (SHA-256 keyed by query + variables + version)
- Token refresh on 401
- Request timing telemetry (`cmd_all_timing_network_ms`)
- `x-request-id` tracking

---

## Domain: Authentication & Session

### `POST {storeFqdn}/admin/oauth/access_token`
- **API type:** REST (OAuth client_credentials grant)
- **Called from:** `packages/cli-kit/src/public/node/session.ts:319` → `ensureAuthenticatedAdminAsApp()`
- **Purpose:** Get an admin API access token using OAuth 2.0 client credentials (client_id + client_secret), used when performing admin operations on behalf of an app (not a user)
- **Request payload:**
  ```json
  {
    "client_id": "<clientId>",
    "client_secret": "<clientSecret>",
    "grant_type": "client_credentials"
  }
  ```
  - Headers: `Content-Type: application/json`
  - Request mode: `'slow-request'` (no retries, no auto-cancellation)
- **Response shape consumed:**
  ```json
  { "access_token": "string" }
  ```
  Mapped to `AdminSession { token, storeFqdn }`.
- **Error handling:** 400 with `app_not_installed` → AbortError with install prompt. Other 400s → AbortError. JSON parse failure → AbortError.

---

## Domain: Organizations

### Partners API — `FindOrganization`
- **API type:** GraphQL (Partners)
- **Called from:** `packages/app/src/cli/services/context/partner-account-info.ts` via PartnersClient
- **Purpose:** Fetch organization details and apps by org ID
- **Full query:**
  ```graphql
  query FindOrganization($id: ID!, $title: String) {
    organizations(id: $id, first: 1) {
      nodes {
        id
        businessName
        apps(first: 25, title: $title) {
          pageInfo { hasNextPage }
          nodes { id, title, apiKey }
        }
      }
    }
  }
  ```
- **Response consumed:** `organizations.nodes[0].{id, businessName, apps.nodes[].{id, title, apiKey}}` — used to populate org selection UI and identify apps within an org.

### Partners API — `findOrgBasic`
- **API type:** GraphQL (Partners)
- **Called from:** `packages/app/src/cli/api/graphql/find_org_basic.ts`
- **Purpose:** Basic org lookup with all apps
- **Response consumed:** Org ID + all apps (id, title, apiKey, etc.)

### Partners API — `AllOrgs`
- **API type:** GraphQL (Partners)
- **Called from:** PartnersClient
- **Purpose:** List all organizations the user has access to
- **Response consumed:** List of orgs with IDs and business names

### Business Platform Organizations — `FindOrganizations`
- **API type:** GraphQL (Business Platform → Organizations)
- **Called from:** AppManagementClient
- **Purpose:** Fetch organizations via the Business Platform API (used for new-style orgs)
- **Endpoint URL:** `https://{businessPlatformFqdn}/organizations/api/unstable/organization/{orgId}/graphql`
- **Response consumed:** Organizations list

### Business Platform Organizations — `OrganizationExpFlags`
- **API type:** GraphQL (Business Platform → Organizations)
- **Called from:** AppManagementClient
- **Purpose:** Get experimental flags for an organization
- **Response consumed:** Feature flags for beta features

### Business Platform Organizations — `FetchStoreByDomain`
- **API type:** GraphQL (Business Platform → Organizations)
- **Called from:** AppManagementClient
- **Purpose:** Look up a store by its domain name
- **Response consumed:** Store details (ID, domain, transferDisabled, etc.)

### Business Platform Organizations — `ListAppDevStores`
- **API type:** GraphQL (Business Platform → Organizations)
- **Called from:** AppManagementClient
- **Purpose:** List development stores for an organization
- **Response consumed:** Paginated list of stores with IDs and domains

### Business Platform Organizations — `ProvisionShopAccess`
- **API type:** GraphQL (Business Platform → Organizations)
- **Called from:** AppManagementClient
- **Purpose:** Ensure a user has access to a development store
- **Response consumed:** Success/error status

---

## Domain: Apps (Partners API — GraphQL)

### `mutation CreateApp`
- **API type:** GraphQL (Partners)
- **Called from:** `packages/app/src/cli/utilities/developer-platform-client/partners-client.ts` → `createApp()`
- **Purpose:** Create a new app in the Partners Dashboard
- **Full query:** `packages/app/src/cli/api/graphql/create_app.ts`
  ```graphql
  mutation CreateApp($org: ID!, $title: String!, $appUrl: Url, $redir: [Url]!, $type: String, $requestedAccessScopes: [String]) {
    appCreate(input: {organizationID: $org, title: $title, applicationUrl: $appUrl, redirectUrlWhitelist: $redir, appType: $type, requestedAccessScopes: $requestedAccessScopes}) {
      app { id, apiKey, apiSecretKeys { secret }, appType }
      userErrors { field, message }
    }
  }
  ```
- **Request payload:** `{ org: ID, title: string, appUrl?, redir: string[], type?, requestedAccessScopes? }`
- **Response consumed:** `appCreate.app.{id, apiKey, apiSecretKeys[].secret, appType}` — used to store API key and secret for the new app.

### `query FindApp`
- **API type:** GraphQL (Partners)
- **Called from:** PartnersClient → `appFromIdentifiers()`, `activeAppVersion()`
- **Purpose:** Fetch app details by API key
- **Full query:** `packages/app/src/cli/api/graphql/find_app.ts`
  ```graphql
  query FindApp($apiKey: String!) {
    app(apiKey: $apiKey) {
      id, title, apiKey, organizationId
      apiSecretKeys { secret }
      appType, grantedScopes, applicationUrl, redirectUrlWhitelist
      requestedAccessScopes, webhookApiVersion, embedded, posEmbedded
      preferencesUrl
      gdprWebhooks { customerDeletionUrl, customerDataRequestUrl, shopDeletionUrl }
      appProxy { subPath, subPathPrefix, url }
      developmentStorePreviewEnabled, disabledFlags
    }
  }
  ```
- **Response consumed:** Full app metadata used to populate `OrganizationApp` model.

### `query allAppExtensionRegistrations`
- **API type:** GraphQL (Partners)
- **Called from:** PartnersClient → `appExtensionRegistrations()` and AppManagementClient → `appExtensionRegistrations()`
- **Purpose:** Get all extension registrations for an app (including config, dashboard-managed)
- **Full query:** `packages/app/src/cli/api/graphql/all_app_extension_registrations.ts`
  ```graphql
  query allAppExtensionRegistrations($apiKey: String!) {
    app(apiKey: $apiKey) {
      extensionRegistrations { id, uuid, title, type, draftVersion { config, context }, activeVersion { config, context } }
      configurationRegistrations { id, uuid, title, type, draftVersion { config, context }, activeVersion { config, context } }
      dashboardManagedExtensionRegistrations { id, uuid, title, type, activeVersion { config, context }, draftVersion { config, context } }
    }
  }
  ```
- **Response consumed:** Three lists of extension registrations, used to match local extensions to remote registrations during dev and deploy.

### `query activeAppVersion`
- **API type:** GraphQL (Partners)
- **Called from:** PartnersClient → `activeAppVersion()`
- **Purpose:** Get the currently active version's module versions
- **Full query:** `packages/app/src/cli/api/graphql/app_active_version.ts`
  ```graphql
  query activeAppVersion($apiKey: String!) {
    app(apiKey: $apiKey) {
      activeAppVersion {
        appModuleVersions {
          registrationId, registrationUuid, registrationTitle, type, config
          specification { identifier, name, experience, options { managementExperience } }
        }
      }
    }
  }
  ```
- **Response consumed:** `appModuleVersions[]` — used to determine what extensions are currently deployed and their configs.

### `query AppVersions`
- **API type:** GraphQL (Partners)
- **Called from:** PartnersClient → `appVersions()`
- **Purpose:** List all versions of an app
- **Full query:** `packages/app/src/cli/api/graphql/get_versions_list.ts`
  ```graphql
  query AppVersions($apiKey: String!) {
    app(apiKey: $apiKey) { appVersions { nodes { uuid, id, message, versionTag, createdAt, appModuleVersions { ... } } } }
  }
  ```
- **Response consumed:** Version list used by `versions list` command.

### `query AppVersionByTag`
- **API type:** GraphQL (Partners)
- **Called from:** PartnersClient → `appVersionByTag()`
- **Purpose:** Fetch a specific version by its version tag
- **Response consumed:** Version details including module versions

### `query AppVersionsDiff`
- **API type:** GraphQL (Partners)
- **Called from:** PartnersClient → `appVersionsDiff()`
- **Purpose:** Get diff between two versions (added/updated/removed extensions)
- **Full query:** `packages/app/src/cli/api/graphql/app_versions_diff.ts`
  ```graphql
  query AppVersionsDiff($apiKey: String!, $versionId: ID!) {
    app(apiKey: $apiKey) {
      versionsDiff(appVersionId: $versionId) {
        added { uuid, registrationTitle, specification { identifier, experience, options { managementExperience } } }
        updated { ... }
        removed { ... }
      }
    }
  }
  ```
- **Response consumed:** Three lists of changes — used to present confirmation UI before releasing.

### `mutation AppDeploy`
- **API type:** GraphQL (Partners)
- **Called from:** PartnersClient → `deploy()` and AppManagementClient → `deploy()`
- **Purpose:** Deploy a new version of an app (upload extension configurations)
- **Full query:** `packages/app/src/cli/api/graphql/app_deploy.ts`
  ```graphql
  mutation AppDeploy($apiKey: String!, $bundleUrl: String, $appModules: [AppModuleSettings!], $skipPublish: Boolean, $message: String, $versionTag: String, $commitReference: String) {
    appDeploy(input: { apiKey: $apiKey, bundleUrl: $bundleUrl, appModules: $appModules, skipPublish: $skipPublish, message: $message, versionTag: $versionTag, commitReference: $commitReference }) {
      appVersion { uuid, id, message, versionTag, location, appModuleVersions { uuid, registrationUuid, validationErrors { message, field } } }
      userErrors { message, field, category, details }
    }
  }
  ```
- **Request payload:** `{ apiKey, bundleUrl?, appModules: [{ uid?, uuid?, specificationIdentifier, config, context, handle }], skipPublish?, message?, versionTag?, commitReference? }`
- **Response consumed:** Created app version with UUID, ID, and per-module validation errors.

### `mutation AppRelease`
- **API type:** GraphQL (Partners)
- **Called from:** PartnersClient → `release()`
- **Purpose:** Release (publish) a previously deployed version
- **Full query:** `packages/app/src/cli/api/graphql/app_release.ts`
  ```graphql
  mutation AppRelease($apiKey: String!, $appVersionId: ID, $versionTag: String) {
    appRelease(input: { apiKey: $apiKey, appVersionId: $appVersionId, versionTag: $versionTag }) {
      appVersion { versionTag, message, location }
      userErrors { message, field, category, details }
    }
  }
  ```
- **Request payload:** `{ apiKey, appVersionId?, versionTag? }`
- **Response consumed:** Released version metadata.

### `mutation UpdateURLs`
- **API type:** GraphQL (Partners)
- **Called from:** PartnersClient → `updateURLs()`
- **Purpose:** Update app URL configuration (app URL, redirect URLs, app proxy)
- **Full query:** `packages/app/src/cli/api/graphql/update_urls.ts`
  ```graphql
  mutation appUpdate($apiKey: String!, $applicationUrl: Url!, $redirectUrlWhitelist: [Url]!, $appProxy: AppProxyInput) {
    appUpdate(input: { apiKey: $apiKey, applicationUrl: $applicationUrl, redirectUrlWhitelist: $redirectUrlWhitelist, appProxy: $appProxy }) {
      userErrors { message, field }
    }
  }
  ```
- **Request payload:** `{ apiKey, applicationUrl, redirectUrlWhitelist, appProxy?: { proxyUrl, proxySubPath, proxySubPathPrefix } }`
- **Response consumed:** User errors array.

### `query fetchSpecifications`
- **API type:** GraphQL (Partners)
- **Called from:** PartnersClient → `specifications()`
- **Purpose:** Get available extension type specifications
- **Full query:** `packages/app/src/cli/api/graphql/extension_specifications.ts`
  ```graphql
  query fetchSpecifications($apiKey: String!) {
    extensionSpecifications(apiKey: $apiKey) {
      name, externalName, externalIdentifier, identifier, gated, experience
      options { managementExperience, registrationLimit }
      features { argo { surface } }
      validationSchema { jsonSchema }
    }
  }
  ```
- **Response consumed:** Array of `RemoteSpecification` — used to validate and configure extension types.

### `mutation ExtensionCreate`
- **API type:** GraphQL (Partners)
- **Called from:** PartnersClient → `createExtension()`
- **Purpose:** Register a new extension in the Partners Dashboard
- **Full query:** `packages/app/src/cli/api/graphql/extension_create.ts`
  ```graphql
  mutation ExtensionCreate($apiKey: String!, $type: ExtensionType!, $title: String!, $config: JSON!, $context: String, $handle: String) {
    extensionCreate(input: { apiKey: $apiKey, type: $type, title: $title, config: $config, context: $context, handle: $handle }) {
      extensionRegistration { id, uuid, type, title, draftVersion { config, registrationId, lastUserInteractionAt, validationErrors { field, message } } }
      userErrors { field, message }
    }
  }
  ```
- **Request payload:** `{ apiKey, type, title, config: JSON-string, context?, handle }`
- **Response consumed:** Created registration with ID, UUID, and draft version info.

### `mutation extensionUpdate` (Partners draft)
- **API type:** GraphQL (Partners)
- **Called from:** PartnersClient → `updateExtension()`
- **Purpose:** Update an extension draft version
- **Query:** `packages/app/src/cli/api/graphql/partners/generated/update-draft.ts`
- **Request payload:** Registration ID + updated config/context
- **Response consumed:** Updated draft version with validation errors

### `mutation GenerateSignedUploadUrl`
- **API type:** GraphQL (Partners)
- **Called from:** PartnersClient → `generateSignedUploadUrl()`
- **Purpose:** Get a signed URL for uploading the app bundle
- **Full query:** `packages/app/src/cli/api/graphql/generate_signed_upload_url.ts`
  ```graphql
  mutation GenerateSignedUploadUrl($apiKey: String!, $bundleFormat: Int!) {
    appVersionGenerateSignedUploadUrl(input: { apiKey: $apiKey, bundleFormat: $bundleFormat }) {
      signedUploadUrl
      userErrors { field, message }
    }
  }
  ```
- **Request payload:** `{ apiKey, bundleFormat: 1 (zip) | 2 (br) }`
- **Response consumed:** `signedUploadUrl` — used to upload the deployment bundle.

### `query currentAccountInfo`
- **API type:** GraphQL (Partners)
- **Called from:** PartnersClient → `currentAccountInfo()`
- **Purpose:** Fetch current user's account info
- **Response consumed:** Account type (User/Service), email, org name

### `mutation convertDevToTestStore`
- **API type:** GraphQL (Partners)
- **Called from:** PartnersClient → `convertToTransferDisabledStore()`
- **Purpose:** Convert a development store to a transfer-disabled (test) store
- **Full query:** `packages/app/src/cli/api/graphql/convert_dev_to_transfer_disabled_store.ts`
  ```graphql
  mutation convertDevToTestStore($input: ConvertDevToTestStoreInput!) {
    convertDevToTestStore(input: $input) {
      convertedToTestStore
      userErrors { message, field }
    }
  }
  ```
- **Request payload:** `{ input: { organizationID, shopId } }`
- **Response consumed:** `convertedToTestStore: boolean`

### `query FindAppPreviewMode`
- **API type:** GraphQL (Partners)
- **Called from:** PartnersClient → `appPreviewMode()`
- **Purpose:** Get the current development store preview mode for an app
- **Response consumed:** Preview mode settings

### `mutation developmentStorePreviewUpdate`
- **API type:** GraphQL (Partners)
- **Called from:** PartnersClient → `updateDeveloperPreview()`
- **Purpose:** Enable/disable development store preview mode
- **Response consumed:** Updated preview mode status

### `mutation ExtensionMigrateFlowExtension`
- **API type:** GraphQL (Partners)
- **Called from:** PartnersClient → `migrateFlowExtension()`
- **Purpose:** Migrate a Flow extension to the new extension model
- **Response consumed:** Migration result with userErrors

### `mutation MigrateAppModule`
- **API type:** GraphQL (Partners)
- **Called from:** PartnersClient → `migrateAppModule()`
- **Purpose:** Migrate an app module to a different type
- **Response consumed:** Migration result

### `mutation MigrateToUiExtension`
- **API type:** GraphQL (Partners)
- **Called from:** PartnersClient → `migrateToUiExtension()`
- **Purpose:** Migrate a legacy extension to UI Extension format
- **Response consumed:** Migration result

### `query templateSpecifications`
- **API type:** GraphQL (Partners)
- **Called from:** PartnersClient → `templateSpecifications()`
- **Purpose:** Get available extension templates for scaffolding
- **Response consumed:** Template list with names, paths, and configuration

### `query FindStoreByDomain`
- **API type:** GraphQL (Partners)
- **Called from:** PartnersClient → `storeByDomain()`
- **Purpose:** Find a store by its domain name
- **Response consumed:** Store details

### `query DevStoresByOrg`
- **API type:** GraphQL (Partners)
- **Called from:** PartnersClient → `devStoresForOrg()`
- **Purpose:** List all development stores for an org
- **Response consumed:** Paginated store list

### `query SchemaDefinitionByTarget`
- **API type:** GraphQL (Partners via Functions API)
- **Called from:** PartnersClient → `targetSchemaDefinition()`
- **Purpose:** Get the function spec schema by deployment target
- **Response consumed:** Schema definition string or null

### `query SchemaDefinitionByApiType`
- **API type:** GraphQL (Partners via Functions API)
- **Called from:** PartnersClient → `apiSchemaDefinition()`
- **Purpose:** Get the function spec schema by API type
- **Response consumed:** Schema definition string or null

---

## Domain: Apps (App Management API — GraphQL)

### `mutation CreateApp` (App Management)
- **API type:** GraphQL (App Management)
- **Called from:** AppManagementClient → `createApp()`
- **Purpose:** Create a new app via App Management API
- **Endpoint URL:** `https://{appManagementFqdn}/app_management/unstable/graphql`
- **Request payload:** `{ initialVersion: AppVersionInput, organizationId: ID }`
- **Response consumed:** `appCreate.app.{id, key, activeRoot.clientCredentials.secrets[].key}` — app ID, API key, and client secret.

### `query specifications` (App Management)
- **API type:** GraphQL (App Management)
- **Called from:** AppManagementClient → `specifications()`
- **Purpose:** Get available extension specifications via App Management API
- **Response consumed:** Same shape as Partners' `extensionSpecifications`

### `query apps` / `query appVersionByTag` / `query appVersions` / `query appVersionById`
- **API type:** GraphQL (App Management)
- **Called from:** AppManagementClient
- **Purpose:** App Management equivalents of the Partners queries for app lookup, version listing, and version retrieval
- **Response consumed:** Same shapes as Partners equivalents

### `query activeAppRelease` / `query activeAppReleaseFromApiKey`
- **API type:** GraphQL (App Management)
- **Called from:** AppManagementClient → `activeAppVersion()`
- **Purpose:** Get the currently active release for an app
- **Response consumed:** Active root version with module versions

### `mutation createAppVersion`
- **API type:** GraphQL (App Management)
- **Called from:** AppManagementClient → `deploy()`
- **Purpose:** Create a new app version (deploy)
- **Response consumed:** Created version with module version UUIDs

### `mutation releaseVersion`
- **API type:** GraphQL (App Management)
- **Called from:** AppManagementClient → `release()`
- **Purpose:** Release a created version
- **Response consumed:** Released version metadata

### `mutation createAssetUrl`
- **API type:** GraphQL (App Management)
- **Called from:** AppManagementClient → `generateSignedUploadUrl()`
- **Purpose:** Get signed upload URL for bundle
- **Response consumed:** Upload URL

### `query appInstallCount`
- **API type:** GraphQL (App Management)
- **Called from:** AppManagementClient → `appInstallCount()`
- **Purpose:** Get the number of stores an app is installed on
- **Response consumed:** Install count number

### `mutation appLogsSubscribe`
- **API type:** GraphQL (App Management)
- **Called from:** AppManagementClient → `subscribeToAppLogs()`
- **Purpose:** Subscribe to app log events
- **Response consumed:** Subscription confirmation

---

## Domain: App Dev Sessions (App Dev API — GraphQL)

### `mutation DevSessionCreate`
- **API type:** GraphQL (App Dev)
- **Called from:** AppManagementClient → `devSessionCreate()`
- **Purpose:** Create a development session for an app on a store
- **Endpoint URL:** `https://{appDevFqdn}/app_dev/unstable/graphql.json`
- **Request payload:** `{ appId: string, assetsUrl: string, websocketUrl?: string }`
- **Response consumed:** `devSessionCreate.devSession.{websocketUrl, updatedAt, user.{id, email}, app.{id, key}}`

### `mutation DevSessionUpdate`
- **API type:** GraphQL (App Dev)
- **Called from:** AppManagementClient → `devSessionUpdate()`
- **Purpose:** Update an existing dev session with new assets/manifest
- **Response consumed:** Updated session data

### `mutation DevSessionDelete`
- **API type:** GraphQL (App Dev)
- **Called from:** AppManagementClient → `devSessionDelete()`
- **Purpose:** Delete a development session
- **Response consumed:** Deletion confirmation

---

## Domain: Webhooks (Partners + App Management)

### Partners API — `sendSampleWebhook` (via `sendSampleWebhook.ts`)
- **API type:** GraphQL (Partners) — raw string query, not gql-tagged
- **Called from:** PartnersClient → `sendSampleWebhook()`
- **Purpose:** Send a sample webhook payload for testing
- **Full query:**
  ```graphql
  mutation samplePayload($topic: String!, $api_version: String!, $address: String!, $delivery_method: String!, $shared_secret: String!, $api_key: String) {
    sendSampleWebhook(input: {topic: $topic, apiVersion: $api_version, address: $address, deliveryMethod: $delivery_method, sharedSecret: $shared_secret, apiKey: $api_key}) {
      samplePayload, success, headers
      userErrors { message }
    }
  }
  ```
- **Request payload:** `{ topic, api_version, address, delivery_method, shared_secret, api_key? }`
- **Response consumed:** `{ samplePayload, success, headers, userErrors }` — used to display the webhook result.

### Partners API — `publicApiVersions` (webhooks)
- **API type:** GraphQL (Partners)
- **Called from:** PartnersClient → `apiVersions()`
- **Purpose:** List available API versions for webhooks
- **Query:** `packages/app/src/cli/services/webhook/request-api-versions.ts`
- **Response consumed:** API version list

### Partners API — `availableTopics`
- **API type:** GraphQL (Partners)
- **Called from:** PartnersClient → `topics()`
- **Purpose:** List available webhook topics for a given API version
- **Query:** `packages/app/src/cli/services/webhook/request-topics.ts`
- **Response consumed:** Topic list

### App Management — Webhooks API
- **API type:** GraphQL (Webhooks via App Management)
- **Called from:** AppManagementClient → `sendSampleWebhook()`, `apiVersions()`, `topics()`
- **Purpose:** Same webhook operations through App Management API
- **Endpoint URL:** `https://{appManagementFqdn}/webhooks/unstable/organizations/{orgId}/graphql.json`
- **Response consumed:** Same shapes as Partners equivalents

---

## Domain: Shopify Functions (App Management — Functions API)

### Functions API — `SchemaDefinitionByTarget`
- **API type:** GraphQL (App Management → Functions)
- **Called from:** AppManagementClient → `targetSchemaDefinition()`
- **Purpose:** Get schema definition for Shopify Functions by deployment target
- **Endpoint URL:** `https://{appManagementFqdn}/functions/unstable/organizations/{orgId}/{appId}/graphql`
- **Query:** `packages/app/src/cli/api/graphql/functions/queries/schema-definition-by-target.graphql`
- **Response consumed:** JSON schema string for the function's input.

### Functions API — `SchemaDefinitionByApiType`
- **API type:** GraphQL (App Management → Functions)
- **Called from:** AppManagementClient → `apiSchemaDefinition()`
- **Purpose:** Get schema definition by API type
- **Query:** `packages/app/src/cli/api/graphql/functions/queries/schema-definition-by-api-type.graphql`
- **Response consumed:** JSON schema string.

---

## Domain: Themes (Admin API — GraphQL)

All theme API calls use the Admin API GraphQL endpoint:
`POST https://{store}/admin/api/{version}/graphql.json`

### `query getTheme`
- **API type:** GraphQL (Admin)
- **Called from:** `packages/cli-kit/src/public/node/themes/api.ts:52` → `fetchTheme()`
- **Purpose:** Fetch a single theme by ID
- **Query:**
  ```graphql
  query getTheme($id: ID!) { theme(id: $id) { id, name, role, processing } }
  ```
- **Response consumed:** `theme.{id, name, role, processing}` → mapped to `Theme` model via `buildTheme()`.

### `query getThemes`
- **API type:** GraphQL (Admin)
- **Called from:** `packages/cli-kit/src/public/node/themes/api.ts:91` → `fetchThemes()`
- **Purpose:** List all themes with cursor-based pagination (50 per page)
- **Query:**
  ```graphql
  query getThemes($after: String) {
    themes(first: 50, after: $after) { nodes { id, name, role, processing } pageInfo { hasNextPage, endCursor } }
  }
  ```
- **Response consumed:** Array of `Theme` models, paginated until `hasNextPage` is false.

### `query findDevelopmentThemeByName`
- **API type:** GraphQL (Admin)
- **Called from:** `packages/cli-kit/src/public/node/themes/api.ts:125` → `findDevelopmentThemeByName()`
- **Purpose:** Find a development theme by exact name
- **Query:**
  ```graphql
  query findDevelopmentThemeByName($name: String!) {
    themes(first: 2, names: [$name], roles: [DEVELOPMENT]) { nodes { id, name, role, processing } }
  }
  ```
- **Response consumed:** Single theme or undefined (errors if >1 found).

### `mutation themeCreate`
- **API type:** GraphQL (Admin)
- **Called from:** `packages/cli-kit/src/public/node/themes/api.ts:156` → `themeCreate()`
- **Purpose:** Create a new theme from a source ZIP URL
- **Query:**
  ```graphql
  mutation themeCreate($name: String!, $source: URL!, $role: ThemeRole!) {
    themeCreate(name: $name, source: $source, role: $role) {
      theme { id, name, role }
      userErrors { field, message }
    }
  }
  ```
- **Request payload:** `{ name, source: URL (defaults to skeleton ZIP from cdn.shopify.com), role }`
- **Response consumed:** Created `Theme` or user errors.

### `mutation themeUpdate`
- **API type:** GraphQL (Admin)
- **Called from:** `packages/cli-kit/src/public/node/themes/api.ts:420` → `themeUpdate()`
- **Purpose:** Update theme metadata (name)
- **Query:**
  ```graphql
  mutation themeUpdate($id: ID!, $input: OnlineStoreThemeInput!) {
    themeUpdate(id: $id, input: $input) { theme { id, name, role } userErrors { field, message } }
  }
  ```
- **Response consumed:** Updated `Theme`.

### `mutation themeDelete`
- **API type:** GraphQL (Admin)
- **Called from:** `packages/cli-kit/src/public/node/themes/api.ts:482` → `themeDelete()`
- **Purpose:** Delete a theme
- **Query:**
  ```graphql
  mutation themeDelete($id: ID!) {
    themeDelete(id: $id) { deletedThemeId userErrors { field, message } }
  }
  ```
- **Response consumed:** `deletedThemeId` or user errors.

### `mutation themeDuplicate`
- **API type:** GraphQL (Admin)
- **Called from:** `packages/cli-kit/src/public/node/themes/api.ts:521` → `themeDuplicate()`
- **Purpose:** Duplicate a theme (optionally with new name)
- **Query:**
  ```graphql
  mutation themeDuplicate($id: ID!, $name: String) {
    themeDuplicate(id: $id, name: $name) { newTheme { id, name, role } userErrors { field, message } }
  }
  ```
- **Response consumed:** `ThemeDuplicateResult` with optional new theme, user errors, and x-request-id.

### `mutation themePublish`
- **API type:** GraphQL (Admin)
- **Called from:** `packages/cli-kit/src/public/node/themes/api.ts:451` → `themePublish()`
- **Purpose:** Publish a theme (set it as the live theme)
- **Query:**
  ```graphql
  mutation themePublish($id: ID!) {
    themePublish(id: $id) { theme { id, name, role } userErrors { field, message } }
  }
  ```
- **Response consumed:** Published `Theme`.

### `query getThemeFileBodies`
- **API type:** GraphQL (Admin)
- **Called from:** `packages/cli-kit/src/public/node/themes/api.ts:196` → `fetchThemeAssets()`
- **Purpose:** Fetch theme file contents by filename (cursor-paginated, 250 per page)
- **Query:**
  ```graphql
  query getThemeFileBodies($id: ID!, $after: String, $filenames: [String!]) {
    theme(id: $id) {
      files(first: 250, after: $after, filenames: $filenames) {
        nodes { filename, size, checksumMd5, body { __typename, ... on OnlineStoreThemeFileBodyText { content }, ... on OnlineStoreThemeFileBodyBase64 { contentBase64 }, ... on OnlineStoreThemeFileBodyUrl { url } } }
        userErrors { filename, code }
        pageInfo { hasNextPage, endCursor }
      }
    }
  }
  ```
- **Response consumed:** File bodies decoded via `parseThemeFileContent()` (handles TEXT, BASE64, and URL types), returned as `ThemeAsset[]`.

### `query getThemeFileChecksums`
- **API type:** GraphQL (Admin)
- **Called from:** `packages/cli-kit/src/public/node/themes/api.ts:382` → `fetchChecksums()`
- **Purpose:** Fetch MD5 checksums of all theme files (for diff detection)
- **Query:**
  ```graphql
  query getThemeFileChecksums($id: ID!, $after: String) {
    theme(id: $id) {
      files(first: 250, after: $after) {
        nodes { filename, size, checksumMd5 }
        userErrors { filename, code }
        pageInfo { hasNextPage, endCursor }
      }
    }
  }
  ```
- **Response consumed:** Array of `{ key, checksum }` pairs — used to determine which files need uploading.

### `mutation themeFilesUpsert`
- **API type:** GraphQL (Admin)
- **Called from:** `packages/cli-kit/src/public/node/themes/api.ts:297` → `uploadFiles()` (called by `bulkUploadThemeAssets()`)
- **Purpose:** Upload or update theme files (batched 50 at a time)
- **Query:**
  ```graphql
  mutation themeFilesUpsert($files: [OnlineStoreThemeFilesUpsertFileInput!]!, $themeId: ID!) {
    themeFilesUpsert(files: $files, themeId: $themeId) {
      upsertedThemeFiles { filename }
      userErrors { filename, message }
    }
  }
  ```
- **Request payload:** `{ themeId: ID, files: [{ filename, body: { type: TEXT | BASE64, value } }] }`
- **Response consumed:** Upload results with success/failure per file.

### `mutation themeFilesDelete`
- **API type:** GraphQL (Admin)
- **Called from:** `packages/cli-kit/src/public/node/themes/api.ts:242` → `deleteThemeAssets()`
- **Purpose:** Delete theme files (batched 50 at a time)
- **Query:**
  ```graphql
  mutation themeFilesDelete($themeId: ID!, $files: [String!]!) {
    themeFilesDelete(themeId: $themeId, files: $files) {
      deletedThemeFiles { filename }
      userErrors { filename, code, message }
    }
  }
  ```
- **Request payload:** `{ themeId: ID, files: string[] }`
- **Response consumed:** `Result[]` with per-file success/failure.

### `query metafieldDefinitionsByOwnerType`
- **API type:** GraphQL (Admin)
- **Called from:** `packages/cli-kit/src/public/node/themes/api.ts:577` → `metafieldDefinitionsByOwnerType()`
- **Purpose:** Get metafield definitions for a given owner type (used for theme metafield support)
- **Query:**
  ```graphql
  query metafieldDefinitionsByOwnerType($ownerType: MetafieldOwnerType!) {
    metafieldDefinitions(ownerType: $ownerType, first: 250) {
      nodes { key, name, namespace, description, type { category, name } }
    }
  }
  ```
- **Response consumed:** Array of `{ key, namespace, name, description, type: { name, category } }`.

### `query OnlineStorePasswordProtection`
- **API type:** GraphQL (Admin)
- **Called from:** `packages/cli-kit/src/public/node/themes/api.ts:597` → `passwordProtected()`
- **Purpose:** Check if the online store has password protection enabled
- **Query:**
  ```graphql
  query OnlineStorePasswordProtection {
    onlineStore { passwordProtection { enabled } }
  }
  ```
- **Response consumed:** `passwordProtection.enabled: boolean` — used to warn users before theme operations.

---

## Domain: Admin API Discovery

### `query publicApiVersions`
- **API type:** GraphQL (Admin)
- **Called from:** `packages/cli-kit/src/public/node/api/admin.ts:158` → `fetchApiVersions()`
- **Purpose:** Discover the latest supported Admin API version for a store
- **Query:**
  ```graphql
  query publicApiVersions { publicApiVersions { handle, supported } }
  ```
- **Response consumed:** Filtered to `supported === true`, sorted alphabetically, latest version used for all subsequent Admin API calls. Version cached per FQDN in `LatestApiVersionByFQDN` map.
- Note: Called with `version: 'unstable'` so it works regardless of store's API version.

---

## Domain: Bulk Operations (Admin API via Partners/App Management)

The CLI supports running GraphQL bulk operations against the Admin API. The flow involves two steps:

### `mutation bulkOperationRunQuery`
- **API type:** GraphQL (App Management / Partners)
- **Called from:** Bulk operations service
- **Purpose:** Start a bulk query operation
- **Query:** `packages/app/src/cli/api/graphql/bulk-operations/mutations/bulk-operation-run-query.graphql`
- **Response consumed:** `bulkOperationRunQuery.bulkOperation.{id, url}`

### `mutation bulkOperationRunMutation`
- **API type:** GraphQL (App Management / Partners)
- **Called from:** Bulk operations service
- **Purpose:** Start a bulk mutation operation
- **Query:** `packages/app/src/cli/api/graphql/bulk-operations/mutations/bulk-operation-run-mutation.graphql`

### `query getBulkOperationById`
- **API type:** GraphQL (App Management / Partners)
- **Called from:** Bulk operations service → `watchBulkOperation()`
- **Purpose:** Poll for bulk operation completion
- **Query:** `packages/app/src/cli/api/graphql/bulk-operations/queries/get-bulk-operation-by-id.graphql`
- **Response consumed:** `node.{id, status, url, objectCount, errorCode}`

### `mutation bulkOperationCancel`
- **API type:** GraphQL (App Management / Partners)
- **Called from:** `commands/app/bulk/cancel.ts`
- **Purpose:** Cancel a running bulk operation
- **Query:** `packages/app/src/cli/api/graphql/bulk-operations/mutations/bulk-operation-cancel.graphql`

### `mutation stagedUploadsCreate`
- **API type:** GraphQL (App Management / Partners)
- **Called from:** Bulk operations service → `stageFile()`
- **Purpose:** Stage a file for bulk operation upload
- **Query:** `packages/app/src/cli/api/graphql/bulk-operations/mutations/staged-uploads-create.graphql`
- **Response consumed:** Upload URL for staging the bulk operation file

---

## Domain: App Logs

### Partners API — `generateFetchAppLogUrl` (HTTP polling, not GraphQL)
- **API type:** REST (HTTPS GET)
- **Called from:** `packages/cli-kit/src/public/node/api/partners.ts:94` via `generateFetchAppLogUrl()`
- **Purpose:** Poll for app log entries at `https://{partnersFqdn}/app_logs/poll`
- **Query params:** `cursor`, `status`, `source` (appended by `addCursorAndFiltersToAppLogsUrl()`)
- **Response consumed:** JSON array of log entries with cursor for next poll.

### App Management API — `appManagementAppLogsUrl` (HTTP polling, not GraphQL)
- **API type:** REST (HTTPS GET)
- **Called from:** `packages/cli-kit/src/public/node/api/app-management.ts:43` via `appManagementAppLogsUrl()`
- **Purpose:** Poll for app log entries at `https://{appManagementFqdn}/app_management/unstable/organizations/{orgId}/app_logs/poll`
- **Query params:** `cursor`, `status`, `source`
- **Response consumed:** Same shape as Partners variant.

---

## Domain: Business Platform — User Info

### Business Platform (Destinations) — `UserInfo`
- **API type:** GraphQL (Business Platform → Destinations)
- **Called from:** AppManagementClient (via `businessPlatformRequestDoc`)
- **Purpose:** Fetch current user's identity and organization membership
- **Endpoint URL:** `https://{businessPlatformFqdn}/destinations/api/2020-07/graphql`
- **Query:** `packages/app/src/cli/api/graphql/business-platform-destinations/queries/user-info.graphql`
- **Response consumed:** User email, organizations list

### Business Platform (Destinations) — `FindOrganizations`
- **API type:** GraphQL (Business Platform → Destinations)
- **Called from:** AppManagementClient
- **Purpose:** Fetch organizations list for Business Platform users
- **Response consumed:** Organizations with IDs and metadata

### Business Platform — `UserEmail` (via GraphQL)
- **API type:** GraphQL (Business Platform → Destinations)
- **Called from:** `packages/cli-kit/src/private/node/api/graphql/business-platform-destinations/user-email.ts`
- **Purpose:** Get user email for identity purposes
- **Response consumed:** User email string

---

## Domain: Business Platform — Organizations

### Business Platform (Organizations) — `OrganizationBetaFlags`
- **API type:** GraphQL (Business Platform → Organizations)
- **Called from:** `app-management-client/graphql/organization_beta_flags.ts` from AppManagementClient
- **Purpose:** Get organization beta feature flags
- **Endpoint URL:** `https://{businessPlatformFqdn}/organizations/api/unstable/organization/{orgId}/graphql`
- **Response consumed:** Beta flag toggles

---

## Non-API External HTTP Calls

### Asset download (theme skeleton)
- **URL:** `https://cdn.shopify.com/static/online-store/theme-skeleton.zip`
- **Called from:** `packages/cli-kit/src/public/node/themes/api.ts:38` (constant `SkeletonThemeCdn`) → `themeCreate()` (default `source`)
- **Purpose:** Download the default theme skeleton when creating a theme without a custom source

### GraphQL endpoint (admin default)
- **URL pattern:** `https://{store}/admin/api/{version}/graphql.json`
- **Also:** If `isThemeAccessSession`, URL becomes `https://{themeKitAccessDomain}/cli/admin/api/{version}/graphql.json`
- **Purpose:** All theme-related Admin GraphQL operations

### Internal error analytics
- **URL:** `https://error-analytics-production.shopifysvc.com`
- **Called from:** `error-handler.ts` → Bugsnag initialization
- **Purpose:** Report unexpected errors to Shopify's internal error tracking
