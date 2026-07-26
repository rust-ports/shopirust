use crate::api::admin::{admin_request_doc, AdminError};
use crate::api::generated::graphql::admin::find_development_theme_by_name::{
    FindDevelopmentThemeByNameResponse, FindDevelopmentThemeByNameThemesNodes,
    FindDevelopmentThemeByNameVariables, FIND_DEVELOPMENT_THEME_BY_NAME_QUERY,
};
use crate::api::generated::graphql::admin::get_theme::{
    GetThemeResponse, GetThemeTheme, GetThemeVariables, GET_THEME_QUERY,
};
use crate::api::generated::graphql::admin::get_theme_file_bodies::{
    GetThemeFileBodiesResponse, GetThemeFileBodiesThemeFilesNodes,
    GetThemeFileBodiesThemeFilesNodesBody, GetThemeFileBodiesVariables,
    GET_THEME_FILE_BODIES_QUERY,
};
use crate::api::generated::graphql::admin::get_theme_file_checksums::{
    GetThemeFileChecksumsResponse, GetThemeFileChecksumsVariables, GET_THEME_FILE_CHECKSUMS_QUERY,
};
use crate::api::generated::graphql::admin::get_themes::{
    GetThemesResponse, GetThemesThemesNodes, GetThemesVariables, GET_THEMES_QUERY,
};
use crate::api::generated::graphql::admin::metafield_definitions_by_owner_type::{
    MetafieldDefinitionsByOwnerTypeResponse, MetafieldDefinitionsByOwnerTypeVariables,
    METAFIELD_DEFINITIONS_BY_OWNER_TYPE_QUERY,
};
use crate::api::generated::graphql::admin::online_store_password_protection::{
    OnlineStorePasswordProtectionResponse, ONLINE_STORE_PASSWORD_PROTECTION_QUERY,
};
use crate::api::generated::graphql::admin::theme_create::{
    ThemeCreateResponse, ThemeCreateThemeCreateTheme, ThemeCreateVariables, THEME_CREATE_MUTATION,
};
use crate::api::generated::graphql::admin::theme_delete::{
    ThemeDeleteResponse, ThemeDeleteVariables, THEME_DELETE_MUTATION,
};
use crate::api::generated::graphql::admin::theme_duplicate::{
    ThemeDuplicateResponse, ThemeDuplicateThemeDuplicateNewTheme, ThemeDuplicateVariables,
    THEME_DUPLICATE_MUTATION,
};
use crate::api::generated::graphql::admin::theme_files_delete::{
    ThemeFilesDeleteResponse, ThemeFilesDeleteVariables, THEME_FILES_DELETE_MUTATION,
};
use crate::api::generated::graphql::admin::theme_files_upsert::{
    ThemeFilesUpsertResponse, ThemeFilesUpsertVariables, THEME_FILES_UPSERT_MUTATION,
};
use crate::api::generated::graphql::admin::theme_publish::{
    ThemePublishResponse, ThemePublishThemePublishTheme, ThemePublishVariables,
    THEME_PUBLISH_MUTATION,
};
use crate::api::generated::graphql::admin::theme_update::{
    ThemeUpdateResponse, ThemeUpdateThemeUpdateTheme, ThemeUpdateVariables, THEME_UPDATE_MUTATION,
};
use crate::api::generated::graphql::admin::types::{
    MetafieldOwnerType, OneOrMany, OnlineStoreThemeFileBodyInput,
    OnlineStoreThemeFileBodyInputType, OnlineStoreThemeFilesUpsertFileInput, OnlineStoreThemeInput,
    ThemeRole,
};
use crate::api::graphql::GraphqlRequestError;
use crate::session::AdminSession;
use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

pub type Key = String;

pub const DEVELOPMENT_THEME_ROLE: &str = "development";
pub const LIVE_THEME_ROLE: &str = "live";
pub const UNPUBLISHED_THEME_ROLE: &str = "unpublished";
pub const SKELETON_THEME_CDN: &str =
    "https://cdn.shopify.com/static/online-store/theme-skeleton.zip";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Theme {
    pub id: i64,
    pub name: String,
    pub created_at_runtime: bool,
    pub processing: bool,
    pub role: String,
    pub src: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Checksum {
    pub key: Key,
    pub checksum: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThemeAssetStats {
    pub mtime: u128,
    pub size: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThemeAsset {
    pub key: Key,
    pub checksum: String,
    pub attachment: Option<String>,
    pub value: Option<String>,
    pub stats: Option<ThemeAssetStats>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum Operation {
    Delete,
    Upload,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThemeOperationErrors {
    pub asset: Option<Vec<String>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThemeOperationResult {
    pub key: Key,
    pub operation: Operation,
    pub success: bool,
    pub errors: Option<ThemeOperationErrors>,
    pub asset: Option<ThemeAsset>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThemeParams {
    pub name: Option<String>,
    pub role: Option<String>,
    pub processing: Option<bool>,
    pub src: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssetParams {
    pub key: Key,
    pub value: Option<String>,
    pub attachment: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThemeDuplicateUserError {
    pub field: Option<Vec<String>>,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThemeDuplicateResult {
    pub theme: Option<Theme>,
    pub user_errors: Vec<ThemeDuplicateUserError>,
    pub request_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MetafieldDefinition {
    pub key: String,
    pub namespace: String,
    pub name: String,
    pub description: Option<String>,
    pub r#type: MetafieldDefinitionType,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MetafieldDefinitionType {
    pub name: String,
    pub category: String,
}

pub fn is_development_theme(theme: &Theme) -> bool {
    theme.role == DEVELOPMENT_THEME_ROLE
}

pub fn compose_theme_gid(id: i64) -> String {
    format!("gid://shopify/OnlineStoreTheme/{id}")
}

pub fn parse_gid(gid: &str) -> std::result::Result<i64, AdminError> {
    gid.rsplit('/')
        .next()
        .and_then(|id| id.parse::<i64>().ok())
        .ok_or_else(|| AdminError::Bug(format!("Invalid GID: {gid}")))
}

pub fn theme_preview_url(theme: &Theme, session: &AdminSession) -> String {
    if theme.role == LIVE_THEME_ROLE {
        format!("https://{}", session.store_fqdn)
    } else {
        format!(
            "https://{}?preview_theme_id={}",
            session.store_fqdn, theme.id
        )
    }
}

pub fn theme_editor_url(theme: &Theme, session: &AdminSession) -> String {
    format!(
        "https://{}/admin/themes/{}/editor",
        session.store_fqdn, theme.id
    )
}

pub fn code_editor_url(theme: &Theme, session: &AdminSession) -> String {
    format!("https://{}/admin/themes/{}", session.store_fqdn, theme.id)
}

pub fn store_admin_url(session: &AdminSession) -> String {
    format!("https://{}/admin", session.store_fqdn)
}

pub fn store_password_page(store_fqdn: &str) -> String {
    format!("https://{store_fqdn}/admin/online_store/preferences")
}

pub async fn fetch_theme(
    id: i64,
    session: &AdminSession,
) -> std::result::Result<Option<Theme>, AdminError> {
    let response: GetThemeResponse = request_theme_admin_doc(
        GET_THEME_QUERY,
        session,
        Some(GetThemeVariables {
            id: compose_theme_gid(id),
        }),
        "Failed to fetch theme",
    )
    .await?;

    response.theme.map(build_theme_from_get_theme).transpose()
}

pub async fn fetch_themes(session: &AdminSession) -> std::result::Result<Vec<Theme>, AdminError> {
    let mut themes = Vec::new();
    let mut after = None;

    loop {
        let response: GetThemesResponse = request_theme_admin_doc(
            GET_THEMES_QUERY,
            session,
            Some(GetThemesVariables {
                after: after.clone(),
            }),
            "Failed to fetch themes",
        )
        .await?;

        let payload = response
            .themes
            .ok_or_else(|| AdminError::Abort("Failed to fetch themes".into(), None))?;

        for theme in payload.nodes {
            themes.push(build_theme_from_get_themes_node(theme)?);
        }

        if !payload.page_info.has_next_page {
            return Ok(themes);
        }

        after = payload.page_info.end_cursor;
    }
}

pub async fn find_development_theme_by_name(
    name: &str,
    session: &AdminSession,
) -> std::result::Result<Option<Theme>, AdminError> {
    let response: FindDevelopmentThemeByNameResponse = request_theme_admin_doc(
        FIND_DEVELOPMENT_THEME_BY_NAME_QUERY,
        session,
        Some(FindDevelopmentThemeByNameVariables {
            name: name.to_string(),
        }),
        "Failed to fetch themes",
    )
    .await?;

    let themes = response
        .themes
        .ok_or_else(|| AdminError::Abort("Failed to fetch themes".into(), None))?;

    if themes.nodes.len() > 1 {
        return Err(AdminError::Abort(
            format!("More than one development theme is named \"{name}\""),
            None,
        ));
    }

    themes
        .nodes
        .into_iter()
        .next()
        .map(build_theme_from_development_node)
        .transpose()
}

pub async fn theme_create(
    params: ThemeParams,
    session: &AdminSession,
) -> std::result::Result<Option<Theme>, AdminError> {
    let response: ThemeCreateResponse = request_theme_admin_doc(
        THEME_CREATE_MUTATION,
        session,
        Some(ThemeCreateVariables {
            name: params.name.unwrap_or_default(),
            source: params.src.unwrap_or_else(|| SKELETON_THEME_CDN.to_string()),
            role: role_from_param(params.role.as_deref())?,
        }),
        "Failed to create theme",
    )
    .await?;

    let payload = response
        .theme_create
        .ok_or_else(|| AdminError::Abort("Failed to create theme".into(), None))?;
    abort_on_user_errors(payload.user_errors.into_iter().map(|error| error.message))?;

    payload
        .theme
        .map(build_theme_from_create_theme)
        .transpose()
        .and_then(|theme| {
            theme
                .ok_or_else(|| AdminError::Abort("Failed to create theme".into(), None))
                .map(Some)
        })
}

pub async fn theme_update(
    id: i64,
    params: ThemeParams,
    session: &AdminSession,
) -> std::result::Result<Option<Theme>, AdminError> {
    let response: ThemeUpdateResponse = request_theme_admin_doc(
        THEME_UPDATE_MUTATION,
        session,
        Some(ThemeUpdateVariables {
            id: compose_theme_gid(id),
            input: OnlineStoreThemeInput { name: params.name },
        }),
        "Failed to update theme",
    )
    .await?;

    let payload = response
        .theme_update
        .ok_or_else(|| AdminError::Abort("Failed to update theme".into(), None))?;
    abort_on_user_errors(payload.user_errors.into_iter().map(|error| error.message))?;

    payload
        .theme
        .map(build_theme_from_update_theme)
        .transpose()
        .and_then(|theme| {
            theme
                .ok_or_else(|| AdminError::Abort("Failed to update theme".into(), None))
                .map(Some)
        })
}

pub async fn theme_publish(
    id: i64,
    session: &AdminSession,
) -> std::result::Result<Option<Theme>, AdminError> {
    let response: ThemePublishResponse = request_theme_admin_doc(
        THEME_PUBLISH_MUTATION,
        session,
        Some(ThemePublishVariables {
            id: compose_theme_gid(id),
        }),
        "Failed to publish theme",
    )
    .await?;

    let payload = response
        .theme_publish
        .ok_or_else(|| AdminError::Abort("Failed to publish theme".into(), None))?;
    abort_on_user_errors(payload.user_errors.into_iter().map(|error| error.message))?;

    payload
        .theme
        .map(build_theme_from_publish_theme)
        .transpose()
        .and_then(|theme| {
            theme
                .ok_or_else(|| AdminError::Abort("Failed to publish theme".into(), None))
                .map(Some)
        })
}

pub async fn theme_delete(
    id: i64,
    session: &AdminSession,
) -> std::result::Result<bool, AdminError> {
    let response: ThemeDeleteResponse = request_theme_admin_doc(
        THEME_DELETE_MUTATION,
        session,
        Some(ThemeDeleteVariables {
            id: compose_theme_gid(id),
        }),
        "Failed to delete theme",
    )
    .await?;

    let payload = response
        .theme_delete
        .ok_or_else(|| AdminError::Abort("Failed to delete theme".into(), None))?;
    abort_on_user_errors(payload.user_errors.into_iter().map(|error| error.message))?;

    if payload.deleted_theme_id.is_none() {
        return Err(AdminError::Abort("Failed to delete theme".into(), None));
    }

    Ok(true)
}

pub async fn theme_duplicate(
    id: i64,
    name: Option<String>,
    session: &AdminSession,
) -> std::result::Result<ThemeDuplicateResult, AdminError> {
    let response: ThemeDuplicateResponse = request_theme_admin_doc(
        THEME_DUPLICATE_MUTATION,
        session,
        Some(ThemeDuplicateVariables {
            id: compose_theme_gid(id),
            name,
        }),
        "Failed to duplicate theme",
    )
    .await?;

    let Some(payload) = response.theme_duplicate else {
        return Ok(ThemeDuplicateResult {
            theme: None,
            user_errors: vec![ThemeDuplicateUserError {
                field: None,
                message: "Failed to duplicate theme".into(),
            }],
            request_id: None,
        });
    };

    if !payload.user_errors.is_empty() {
        return Ok(ThemeDuplicateResult {
            theme: None,
            user_errors: payload
                .user_errors
                .into_iter()
                .map(|error| ThemeDuplicateUserError {
                    field: error.field,
                    message: error.message,
                })
                .collect(),
            request_id: None,
        });
    }

    let theme = payload
        .new_theme
        .map(build_theme_from_duplicate_theme)
        .transpose()?;

    Ok(ThemeDuplicateResult {
        theme,
        user_errors: vec![],
        request_id: None,
    })
}

pub async fn fetch_theme_assets(
    id: i64,
    filenames: Vec<Key>,
    session: &AdminSession,
) -> std::result::Result<Vec<ThemeAsset>, AdminError> {
    let mut assets = Vec::new();
    let mut after = None;

    loop {
        let response: GetThemeFileBodiesResponse = request_theme_admin_doc(
            GET_THEME_FILE_BODIES_QUERY,
            session,
            Some(GetThemeFileBodiesVariables {
                id: compose_theme_gid(id),
                filenames: Some(OneOrMany::Many(filenames.clone())),
                after: after.clone(),
            }),
            "Failed to fetch theme assets",
        )
        .await?;

        let files = response
            .theme
            .and_then(|theme| theme.files)
            .ok_or_else(|| AdminError::Abort("Error fetching assets".into(), None))?;

        for file in files.nodes {
            assets.push(build_theme_asset_from_file_body(file).await?);
        }

        if !files.page_info.has_next_page {
            return Ok(assets);
        }

        after = files.page_info.end_cursor;
    }
}

pub async fn fetch_checksums(
    id: i64,
    session: &AdminSession,
) -> std::result::Result<Vec<Checksum>, AdminError> {
    let mut checksums = Vec::new();
    let mut after = None;

    loop {
        let response: GetThemeFileChecksumsResponse = request_theme_admin_doc(
            GET_THEME_FILE_CHECKSUMS_QUERY,
            session,
            Some(GetThemeFileChecksumsVariables {
                id: compose_theme_gid(id),
                after: after.clone(),
            }),
            "Failed to fetch checksums",
        )
        .await?;

        let files = response
            .theme
            .and_then(|theme| theme.files)
            .ok_or_else(|| AdminError::Abort("Failed to fetch checksums".into(), None))?;

        checksums.extend(files.nodes.into_iter().map(|file| Checksum {
            key: file.filename,
            checksum: file.checksum_md5.unwrap_or_default(),
        }));

        if !files.page_info.has_next_page {
            return Ok(checksums);
        }

        after = files.page_info.end_cursor;
    }
}

pub async fn delete_theme_assets(
    id: i64,
    filenames: Vec<Key>,
    session: &AdminSession,
) -> std::result::Result<Vec<ThemeOperationResult>, AdminError> {
    let mut results = Vec::new();

    for batch in filenames.chunks(50) {
        let response: ThemeFilesDeleteResponse = request_theme_admin_doc(
            THEME_FILES_DELETE_MUTATION,
            session,
            Some(ThemeFilesDeleteVariables {
                theme_id: compose_theme_gid(id),
                files: OneOrMany::Many(batch.to_vec()),
            }),
            "Failed to delete theme assets",
        )
        .await?;

        let payload = response
            .theme_files_delete
            .ok_or_else(|| AdminError::Abort("Failed to delete theme assets".into(), None))?;

        if let Some(deleted) = payload.deleted_theme_files {
            results.extend(deleted.into_iter().map(|file| ThemeOperationResult {
                key: file.filename,
                success: true,
                operation: Operation::Delete,
                errors: None,
                asset: None,
            }));
        }

        for error in payload.user_errors {
            let Some(filename) = error.filename else {
                return Err(AdminError::Abort(
                    format!("Failed to delete theme assets: {}", error.message),
                    None,
                ));
            };
            results.push(ThemeOperationResult {
                key: filename,
                success: false,
                operation: Operation::Delete,
                errors: Some(ThemeOperationErrors {
                    asset: Some(vec![error.message]),
                }),
                asset: None,
            });
        }
    }

    Ok(results)
}

pub async fn bulk_upload_theme_assets(
    id: i64,
    assets: Vec<AssetParams>,
    session: &AdminSession,
) -> std::result::Result<Vec<ThemeOperationResult>, AdminError> {
    let mut results = Vec::new();

    for chunk in assets.chunks(50) {
        let files = prepare_files_for_upload(chunk);
        let response: ThemeFilesUpsertResponse = request_theme_admin_doc(
            THEME_FILES_UPSERT_MUTATION,
            session,
            Some(ThemeFilesUpsertVariables {
                files: OneOrMany::Many(files),
                theme_id: compose_theme_gid(id),
            }),
            "Failed to upload theme files",
        )
        .await?;

        results.extend(process_upload_results(response)?);
    }

    Ok(results)
}

pub async fn metafield_definitions_by_owner_type(
    owner_type: MetafieldOwnerType,
    session: &AdminSession,
) -> std::result::Result<Vec<MetafieldDefinition>, AdminError> {
    let response: MetafieldDefinitionsByOwnerTypeResponse = request_theme_admin_doc(
        METAFIELD_DEFINITIONS_BY_OWNER_TYPE_QUERY,
        session,
        Some(MetafieldDefinitionsByOwnerTypeVariables { owner_type }),
        "Failed to fetch metafield definitions",
    )
    .await?;

    Ok(response
        .metafield_definitions
        .nodes
        .into_iter()
        .map(|definition| MetafieldDefinition {
            key: definition.key,
            namespace: definition.namespace,
            name: definition.name,
            description: definition.description,
            r#type: MetafieldDefinitionType {
                name: definition.r#type.name,
                category: definition.r#type.category,
            },
        })
        .collect())
}

pub async fn password_protected(session: &AdminSession) -> std::result::Result<bool, AdminError> {
    let response: OnlineStorePasswordProtectionResponse = request_theme_admin_doc(
        ONLINE_STORE_PASSWORD_PROTECTION_QUERY,
        session,
        None::<serde_json::Value>,
        "Unable to get details about the storefront's password protection",
    )
    .await?;

    Ok(response.online_store.password_protection.enabled)
}

pub async fn parse_theme_file_content(
    body: GetThemeFileBodiesThemeFilesNodesBody,
) -> std::result::Result<(Option<String>, Option<String>), AdminError> {
    match body {
        GetThemeFileBodiesThemeFilesNodesBody::OnlineStoreThemeFileBodyText(body) => {
            Ok((Some(body.content), None))
        }
        GetThemeFileBodiesThemeFilesNodesBody::OnlineStoreThemeFileBodyBase64(body) => {
            Ok((None, Some(body.content_base64)))
        }
        GetThemeFileBodiesThemeFilesNodesBody::OnlineStoreThemeFileBodyUrl(body) => {
            let bytes = reqwest::get(&body.url)
                .await
                .map_err(|_| {
                    AdminError::Abort(
                        format!("Error downloading content from URL: {}", body.url),
                        None,
                    )
                })?
                .bytes()
                .await
                .map_err(|_| {
                    AdminError::Abort(
                        format!("Error downloading content from URL: {}", body.url),
                        None,
                    )
                })?;
            Ok((None, Some(base64_encode(bytes.as_ref()))))
        }
    }
}

fn prepare_files_for_upload(assets: &[AssetParams]) -> Vec<OnlineStoreThemeFilesUpsertFileInput> {
    assets
        .iter()
        .map(|asset| {
            let body = if let Some(attachment) = &asset.attachment {
                OnlineStoreThemeFileBodyInput {
                    r#type: OnlineStoreThemeFileBodyInputType::Base64,
                    value: attachment.clone(),
                }
            } else {
                OnlineStoreThemeFileBodyInput {
                    r#type: OnlineStoreThemeFileBodyInputType::Text,
                    value: asset.value.clone().unwrap_or_default(),
                }
            };

            OnlineStoreThemeFilesUpsertFileInput {
                filename: asset.key.clone(),
                body,
            }
        })
        .collect()
}

fn process_upload_results(
    response: ThemeFilesUpsertResponse,
) -> std::result::Result<Vec<ThemeOperationResult>, AdminError> {
    let payload = response
        .theme_files_upsert
        .ok_or_else(|| AdminError::Abort("Failed to upload theme files".into(), None))?;
    let mut results = Vec::new();

    if let Some(upserted) = payload.upserted_theme_files {
        results.extend(upserted.into_iter().map(|file| ThemeOperationResult {
            key: file.filename,
            success: true,
            operation: Operation::Upload,
            errors: None,
            asset: None,
        }));
    }

    for error in payload.user_errors {
        let Some(filename) = error.filename else {
            return Err(AdminError::Abort(
                format!("Error uploading theme files: {}", error.message),
                None,
            ));
        };
        results.push(ThemeOperationResult {
            key: filename,
            success: false,
            operation: Operation::Upload,
            errors: Some(ThemeOperationErrors {
                asset: Some(vec![error.message]),
            }),
            asset: None,
        });
    }

    Ok(results)
}

async fn request_theme_admin_doc<T, V>(
    query: &str,
    session: &AdminSession,
    variables: Option<V>,
    context: &str,
) -> std::result::Result<T, AdminError>
where
    T: serde::de::DeserializeOwned,
    V: serde::Serialize,
{
    admin_request_doc(query, session, variables)
        .await
        .map_err(|error| graphql_to_admin_error(context, error))
}

fn graphql_to_admin_error(context: &str, error: GraphqlRequestError) -> AdminError {
    AdminError::Abort(format!("{context}: {error}"), None)
}

fn abort_on_user_errors(
    messages: impl Iterator<Item = String>,
) -> std::result::Result<(), AdminError> {
    let messages = messages.collect::<Vec<_>>();
    if messages.is_empty() {
        Ok(())
    } else {
        Err(AdminError::Abort(messages.join(", "), None))
    }
}

fn build_theme(
    id: i64,
    name: String,
    role: ThemeRole,
    processing: Option<bool>,
    created_at_runtime: Option<bool>,
    src: Option<String>,
) -> Theme {
    Theme {
        id,
        name,
        created_at_runtime: created_at_runtime.unwrap_or(false),
        processing: processing.unwrap_or(false),
        role: domain_role(role),
        src,
    }
}

fn build_theme_from_get_theme(theme: GetThemeTheme) -> std::result::Result<Theme, AdminError> {
    Ok(build_theme(
        parse_gid(&theme.id)?,
        theme.name,
        theme.role,
        Some(theme.processing),
        None,
        None,
    ))
}

fn build_theme_from_get_themes_node(
    theme: GetThemesThemesNodes,
) -> std::result::Result<Theme, AdminError> {
    Ok(build_theme(
        parse_gid(&theme.id)?,
        theme.name,
        theme.role,
        Some(theme.processing),
        None,
        None,
    ))
}

fn build_theme_from_development_node(
    theme: FindDevelopmentThemeByNameThemesNodes,
) -> std::result::Result<Theme, AdminError> {
    Ok(build_theme(
        parse_gid(&theme.id)?,
        theme.name,
        theme.role,
        Some(theme.processing),
        None,
        None,
    ))
}

fn build_theme_from_create_theme(
    theme: ThemeCreateThemeCreateTheme,
) -> std::result::Result<Theme, AdminError> {
    Ok(build_theme(
        parse_gid(&theme.id)?,
        theme.name,
        theme.role,
        None,
        None,
        None,
    ))
}

fn build_theme_from_update_theme(
    theme: ThemeUpdateThemeUpdateTheme,
) -> std::result::Result<Theme, AdminError> {
    Ok(build_theme(
        parse_gid(&theme.id)?,
        theme.name,
        theme.role,
        None,
        None,
        None,
    ))
}

fn build_theme_from_publish_theme(
    theme: ThemePublishThemePublishTheme,
) -> std::result::Result<Theme, AdminError> {
    Ok(build_theme(
        parse_gid(&theme.id)?,
        theme.name,
        theme.role,
        None,
        None,
        None,
    ))
}

fn build_theme_from_duplicate_theme(
    theme: ThemeDuplicateThemeDuplicateNewTheme,
) -> std::result::Result<Theme, AdminError> {
    Ok(build_theme(
        parse_gid(&theme.id)?,
        theme.name,
        theme.role,
        None,
        None,
        None,
    ))
}

async fn build_theme_asset_from_file_body(
    file: GetThemeFileBodiesThemeFilesNodes,
) -> std::result::Result<ThemeAsset, AdminError> {
    let (value, attachment) = parse_theme_file_content(file.body).await?;
    let size = value
        .as_deref()
        .or(attachment.as_deref())
        .unwrap_or_default()
        .len();

    Ok(ThemeAsset {
        key: file.filename,
        checksum: file.checksum_md5.unwrap_or_default(),
        attachment,
        value,
        stats: Some(ThemeAssetStats {
            size,
            mtime: now_millis(),
        }),
    })
}

fn role_from_param(role: Option<&str>) -> std::result::Result<ThemeRole, AdminError> {
    match role
        .unwrap_or(DEVELOPMENT_THEME_ROLE)
        .to_ascii_lowercase()
        .as_str()
    {
        "archived" => Ok(ThemeRole::Archived),
        "demo" => Ok(ThemeRole::Demo),
        "development" => Ok(ThemeRole::Development),
        "locked" => Ok(ThemeRole::Locked),
        "live" | "main" => Ok(ThemeRole::Main),
        "mobile" => Ok(ThemeRole::Mobile),
        "unpublished" => Ok(ThemeRole::Unpublished),
        other => Err(AdminError::Abort(
            format!("Invalid theme role: {other}"),
            None,
        )),
    }
}

fn domain_role(role: ThemeRole) -> String {
    match role {
        ThemeRole::Archived => "archived",
        ThemeRole::Demo => "demo",
        ThemeRole::Development => DEVELOPMENT_THEME_ROLE,
        ThemeRole::Locked => "locked",
        ThemeRole::Main => LIVE_THEME_ROLE,
        ThemeRole::Mobile => "mobile",
        ThemeRole::Unpublished => UNPUBLISHED_THEME_ROLE,
    }
    .to_string()
}

fn now_millis() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or_default()
}

fn base64_encode(bytes: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);

    for chunk in bytes.chunks(3) {
        let b0 = chunk[0];
        let b1 = *chunk.get(1).unwrap_or(&0);
        let b2 = *chunk.get(2).unwrap_or(&0);

        out.push(TABLE[(b0 >> 2) as usize] as char);
        out.push(TABLE[(((b0 & 0b0000_0011) << 4) | (b1 >> 4)) as usize] as char);

        if chunk.len() > 1 {
            out.push(TABLE[(((b1 & 0b0000_1111) << 2) | (b2 >> 6)) as usize] as char);
        } else {
            out.push('=');
        }

        if chunk.len() > 2 {
            out.push(TABLE[(b2 & 0b0011_1111) as usize] as char);
        } else {
            out.push('=');
        }
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compose_and_parse_theme_gid() {
        let gid = compose_theme_gid(123);
        assert_eq!(gid, "gid://shopify/OnlineStoreTheme/123");
        assert_eq!(parse_gid(&gid).unwrap(), 123);
    }

    #[test]
    fn parse_gid_rejects_invalid_gid() {
        assert!(parse_gid("not-a-gid").is_err());
    }

    #[test]
    fn main_role_maps_to_live_domain_role() {
        assert_eq!(domain_role(ThemeRole::Main), LIVE_THEME_ROLE);
    }

    #[test]
    fn theme_urls_match_original_cli() {
        let session = AdminSession {
            token: "token".into(),
            store_fqdn: "store.myshopify.com".into(),
        };
        let live_theme = Theme {
            id: 1,
            name: "Live".into(),
            created_at_runtime: false,
            processing: false,
            role: LIVE_THEME_ROLE.into(),
            src: None,
        };
        let dev_theme = Theme {
            role: DEVELOPMENT_THEME_ROLE.into(),
            ..live_theme.clone()
        };

        assert_eq!(
            theme_preview_url(&live_theme, &session),
            "https://store.myshopify.com"
        );
        assert_eq!(
            theme_preview_url(&dev_theme, &session),
            "https://store.myshopify.com?preview_theme_id=1"
        );
        assert_eq!(
            theme_editor_url(&dev_theme, &session),
            "https://store.myshopify.com/admin/themes/1/editor"
        );
        assert_eq!(
            code_editor_url(&dev_theme, &session),
            "https://store.myshopify.com/admin/themes/1"
        );
        assert_eq!(
            store_admin_url(&session),
            "https://store.myshopify.com/admin"
        );
        assert_eq!(
            store_password_page(&session.store_fqdn),
            "https://store.myshopify.com/admin/online_store/preferences"
        );
    }

    #[test]
    fn prepare_files_for_upload_uses_base64_for_attachments() {
        let files = prepare_files_for_upload(&[AssetParams {
            key: "assets/logo.png".into(),
            value: None,
            attachment: Some("abc".into()),
        }]);

        assert_eq!(files[0].filename, "assets/logo.png");
        assert!(matches!(
            files[0].body.r#type,
            OnlineStoreThemeFileBodyInputType::Base64
        ));
        assert_eq!(files[0].body.value, "abc");
    }

    #[test]
    fn prepare_files_for_upload_uses_text_for_values() {
        let files = prepare_files_for_upload(&[AssetParams {
            key: "templates/index.json".into(),
            value: Some("{}".into()),
            attachment: None,
        }]);

        assert_eq!(files[0].filename, "templates/index.json");
        assert!(matches!(
            files[0].body.r#type,
            OnlineStoreThemeFileBodyInputType::Text
        ));
        assert_eq!(files[0].body.value, "{}");
    }

    #[test]
    fn base64_encoder_handles_padding() {
        assert_eq!(base64_encode(b""), "");
        assert_eq!(base64_encode(b"f"), "Zg==");
        assert_eq!(base64_encode(b"fo"), "Zm8=");
        assert_eq!(base64_encode(b"foo"), "Zm9v");
    }
}
