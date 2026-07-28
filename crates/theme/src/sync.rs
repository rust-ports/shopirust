use crate::checksum::{reject_generated_static_assets, Checksum};
use crate::filesystem::{ThemeAsset, ThemeFileSystem, ThemeFsError};
use crate::ignore::{apply_ignore_filters, IgnoreFilters, ThemeFileKey};
use async_trait::async_trait;
use std::collections::{BTreeMap, BTreeSet};

pub const DOWNLOAD_BATCH_SIZE: usize = 50;
pub const MUTATION_BATCH_SIZE: usize = 20;
pub const UPLOAD_BATCH_BYTES: usize = 1024 * 1024;
pub const MAX_UPLOAD_ATTEMPTS: usize = 3;
pub const MINIMUM_THEME_FILES: [&str; 3] = [
    "config/settings_data.json",
    "config/settings_schema.json",
    "layout/theme.liquid",
];

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SyncOptions {
    pub nodelete: bool,
    pub filters: IgnoreFilters,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileOperation {
    Upload,
    Download,
    Delete,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileOperationReport {
    pub key: String,
    pub operation: FileOperation,
    pub success: bool,
    pub errors: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SyncReport {
    pub files: Vec<FileOperationReport>,
}

impl SyncReport {
    pub fn has_failures(&self) -> bool {
        self.files.iter().any(|file| !file.success)
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PullPlan {
    pub download: Vec<String>,
    pub delete: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PushPlan {
    pub upload: Vec<ThemeAsset>,
    pub delete: Vec<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum SyncError {
    #[error("{0}")]
    Remote(String),
    #[error(transparent)]
    FileSystem(#[from] ThemeFsError),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteResult {
    pub key: String,
    pub success: bool,
    pub errors: Vec<String>,
}

#[async_trait]
pub trait ThemeSyncAdmin {
    async fn fetch_checksums(&self, theme_id: i64) -> Result<Vec<Checksum>, SyncError>;
    async fn fetch_assets(
        &self,
        theme_id: i64,
        keys: Vec<String>,
    ) -> Result<Vec<ThemeAsset>, SyncError>;
    async fn upload_assets(
        &self,
        theme_id: i64,
        assets: Vec<ThemeAsset>,
    ) -> Result<Vec<RemoteResult>, SyncError>;
    async fn delete_assets(
        &self,
        theme_id: i64,
        keys: Vec<String>,
    ) -> Result<Vec<RemoteResult>, SyncError>;
}

pub fn plan_pull(
    local: &ThemeFileSystem,
    remote: Vec<Checksum>,
    options: &SyncOptions,
) -> PullPlan {
    let remote = filtered_checksums(remote, &local.filters, &options.filters);
    let remote_by_key: BTreeMap<&str, &str> = remote
        .iter()
        .map(|item| (item.key.as_str(), item.checksum.as_str()))
        .collect();
    let download = remote
        .iter()
        .filter(|item| {
            local
                .files
                .get(&item.key)
                .map_or(true, |local| local.checksum != item.checksum)
        })
        .map(|item| item.key.clone())
        .collect();
    let delete = if options.nodelete {
        Vec::new()
    } else {
        filtered_local(local, &options.filters)
            .into_iter()
            .filter(|key| !remote_by_key.contains_key(key.as_str()))
            .collect()
    };
    PullPlan { download, delete }
}

pub fn plan_push(
    local: &ThemeFileSystem,
    remote: Vec<Checksum>,
    options: &SyncOptions,
) -> PushPlan {
    let remote = filtered_checksums(remote, &local.filters, &options.filters);
    let remote_by_key: BTreeMap<&str, &str> = remote
        .iter()
        .map(|item| (item.key.as_str(), item.checksum.as_str()))
        .collect();
    let local_files: BTreeMap<_, _> =
        apply_ignore_filters(local.files.values().cloned().collect(), &options.filters)
            .into_iter()
            .map(|asset| (asset.key.clone(), asset))
            .collect();
    let mut upload: Vec<_> = local_files
        .values()
        .filter(|asset| {
            remote_by_key
                .get(asset.key.as_str())
                .map_or(true, |checksum| *checksum != asset.checksum.as_str())
        })
        .cloned()
        .collect();
    for key in MINIMUM_THEME_FILES {
        if !remote_by_key.contains_key(key) && !local_files.contains_key(key) {
            let value = if key == "config/settings_schema.json" {
                "[]"
            } else if key == "config/settings_data.json" {
                "{}"
            } else {
                ""
            }
            .to_string();
            upload.push(ThemeAsset {
                key: key.into(),
                checksum: crate::checksum::calculate_checksum(key, Some(value.clone().into())),
                attachment: None,
                value: Some(value),
                stats: None,
            });
        }
    }
    let delete = if options.nodelete {
        Vec::new()
    } else {
        remote
            .into_iter()
            .filter(|item| {
                !MINIMUM_THEME_FILES.contains(&item.key.as_str())
                    && !local_files.contains_key(&item.key)
            })
            .map(|item| item.key)
            .collect()
    };
    PushPlan { upload, delete }
}

fn filtered_checksums(
    remote: Vec<Checksum>,
    mounted: &IgnoreFilters,
    explicit: &IgnoreFilters,
) -> Vec<Checksum> {
    let remote = reject_generated_static_assets(remote);
    let remote = apply_ignore_filters(remote, mounted);
    apply_ignore_filters(remote, explicit)
}

impl ThemeFileKey for Checksum {
    fn key(&self) -> &str {
        &self.key
    }
}

fn filtered_local(local: &ThemeFileSystem, explicit: &IgnoreFilters) -> Vec<String> {
    apply_ignore_filters(local.files.keys().cloned().collect(), explicit)
}

pub fn batches<T: Clone>(items: &[T], size: usize) -> Vec<Vec<T>> {
    items.chunks(size).map(<[T]>::to_vec).collect()
}

pub fn upload_batches(assets: &[ThemeAsset]) -> Vec<Vec<ThemeAsset>> {
    let mut result = Vec::new();
    let mut batch = Vec::new();
    let mut bytes = 0;
    for asset in assets {
        let size = asset.value.as_ref().map_or(0, String::len)
            + asset.attachment.as_ref().map_or(0, String::len);
        if !batch.is_empty()
            && (batch.len() >= MUTATION_BATCH_SIZE || bytes + size > UPLOAD_BATCH_BYTES)
        {
            result.push(std::mem::take(&mut batch));
            bytes = 0;
        }
        bytes += size;
        batch.push(asset.clone());
    }
    if !batch.is_empty() {
        result.push(batch);
    }
    result
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum AssetGroup {
    Independent,
    SettingsSchema,
    Layout,
    Block,
    LiquidSection,
    JsonSection,
    JsonTemplate,
    ContextualJsonTemplate,
    SettingsData,
}

pub fn classify(key: &str) -> AssetGroup {
    if key == "config/settings_schema.json" {
        AssetGroup::SettingsSchema
    } else if key == "config/settings_data.json" {
        AssetGroup::SettingsData
    } else if key.starts_with("layout/") {
        AssetGroup::Layout
    } else if key.starts_with("blocks/") {
        AssetGroup::Block
    } else if key.starts_with("sections/") && key.ends_with(".liquid") {
        AssetGroup::LiquidSection
    } else if key.starts_with("sections/") && key.ends_with(".json") {
        AssetGroup::JsonSection
    } else if key.starts_with("templates/") && key.ends_with(".context.")
        || key.contains(".context.") && key.starts_with("templates/")
    {
        AssetGroup::ContextualJsonTemplate
    } else if key.starts_with("templates/") && key.ends_with(".json") {
        AssetGroup::JsonTemplate
    } else {
        AssetGroup::Independent
    }
}

pub fn ordered_upload_groups(assets: Vec<ThemeAsset>) -> Vec<Vec<ThemeAsset>> {
    let mut groups: BTreeMap<AssetGroup, Vec<ThemeAsset>> = BTreeMap::new();
    for asset in assets {
        groups.entry(classify(&asset.key)).or_default().push(asset);
    }
    groups.into_values().collect()
}

pub fn ordered_deletions(mut keys: Vec<String>) -> Vec<String> {
    keys.sort_by_key(|key| std::cmp::Reverse(classify(key)));
    keys
}

pub async fn pull<A: ThemeSyncAdmin + Sync>(
    api: &A,
    theme_id: i64,
    fs: &mut ThemeFileSystem,
    options: &SyncOptions,
) -> Result<SyncReport, SyncError> {
    let plan = plan_pull(fs, api.fetch_checksums(theme_id).await?, options);
    let mut report = SyncReport::default();
    for batch in batches(&plan.download, DOWNLOAD_BATCH_SIZE) {
        let requested: BTreeSet<_> = batch.iter().cloned().collect();
        let assets = api.fetch_assets(theme_id, batch).await?;
        let received: BTreeSet<_> = assets.iter().map(|asset| asset.key.clone()).collect();
        for asset in assets {
            fs.write(&asset)?;
            report.files.push(ok(asset.key, FileOperation::Download));
        }
        for key in requested.difference(&received) {
            report.files.push(failed(
                key.clone(),
                FileOperation::Download,
                "The remote file was not returned",
            ));
        }
    }
    for key in plan.delete {
        fs.delete(&key)?;
        report.files.push(ok(key, FileOperation::Delete));
    }
    Ok(report)
}

pub async fn push<A: ThemeSyncAdmin + Sync>(
    api: &A,
    theme_id: i64,
    fs: &ThemeFileSystem,
    options: &SyncOptions,
) -> Result<SyncReport, SyncError> {
    let plan = plan_push(fs, api.fetch_checksums(theme_id).await?, options);
    let mut report = SyncReport::default();
    for group in ordered_upload_groups(plan.upload) {
        for batch in upload_batches(&group) {
            reconcile_upload(api, theme_id, batch, &mut report).await?;
        }
    }
    for batch in batches(&ordered_deletions(plan.delete), MUTATION_BATCH_SIZE) {
        for result in api.delete_assets(theme_id, batch).await? {
            report
                .files
                .push(from_remote(result, FileOperation::Delete));
        }
    }
    Ok(report)
}

async fn reconcile_upload<A: ThemeSyncAdmin + Sync>(
    api: &A,
    theme_id: i64,
    mut pending: Vec<ThemeAsset>,
    report: &mut SyncReport,
) -> Result<(), SyncError> {
    let mut final_results = BTreeMap::new();
    for _ in 0..MAX_UPLOAD_ATTEMPTS {
        let results = api.upload_assets(theme_id, pending.clone()).await?;
        for result in results {
            final_results.insert(result.key.clone(), result);
        }
        pending.retain(|asset| {
            final_results
                .get(&asset.key)
                .map_or(true, |result| !result.success)
        });
        if pending.is_empty() {
            break;
        }
    }
    for asset in pending {
        final_results
            .entry(asset.key.clone())
            .or_insert(RemoteResult {
                key: asset.key,
                success: false,
                errors: vec!["Upload failed".into()],
            });
    }
    report.files.extend(
        final_results
            .into_values()
            .map(|result| from_remote(result, FileOperation::Upload)),
    );
    Ok(())
}

fn ok(key: String, operation: FileOperation) -> FileOperationReport {
    FileOperationReport {
        key,
        operation,
        success: true,
        errors: vec![],
    }
}
fn failed(key: String, operation: FileOperation, error: &str) -> FileOperationReport {
    FileOperationReport {
        key,
        operation,
        success: false,
        errors: vec![error.into()],
    }
}
fn from_remote(result: RemoteResult, operation: FileOperation) -> FileOperationReport {
    FileOperationReport {
        key: result.key,
        operation,
        success: result.success,
        errors: result.errors,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    fn asset(key: &str, size: usize) -> ThemeAsset {
        ThemeAsset {
            key: key.into(),
            checksum: key.into(),
            value: Some("x".repeat(size)),
            attachment: None,
            stats: None,
        }
    }
    #[test]
    fn batches_uploads_by_count() {
        assert_eq!(
            upload_batches(
                &(0..21)
                    .map(|i| asset(&format!("assets/{i}.js"), 1))
                    .collect::<Vec<_>>()
            )
            .iter()
            .map(Vec::len)
            .collect::<Vec<_>>(),
            vec![20, 1]
        );
    }
    #[test]
    fn batches_uploads_before_byte_limit() {
        assert_eq!(
            upload_batches(&[asset("assets/a.js", 700_000), asset("assets/b.js", 400_000)]).len(),
            2
        );
    }
    #[test]
    fn dependencies_are_ordered() {
        let groups = ordered_upload_groups(vec![
            asset("config/settings_data.json", 1),
            asset("layout/theme.liquid", 1),
            asset("assets/a.js", 1),
        ]);
        assert_eq!(
            groups
                .iter()
                .map(|g| classify(&g[0].key))
                .collect::<Vec<_>>(),
            vec![
                AssetGroup::Independent,
                AssetGroup::Layout,
                AssetGroup::SettingsData
            ]
        );
    }

    #[test]
    fn plan_push_creates_missing_minimum_assets() {
        let fs = ThemeFileSystem {
            root: std::path::PathBuf::new(),
            files: BTreeMap::new(),
            filters: IgnoreFilters::default(),
        };

        let plan = plan_push(&fs, Vec::new(), &SyncOptions::default());

        let keys = plan
            .upload
            .iter()
            .map(|asset| asset.key.as_str())
            .collect::<Vec<_>>();
        assert_eq!(keys, MINIMUM_THEME_FILES);
        assert!(plan.delete.is_empty());
    }

    #[test]
    fn plan_push_never_deletes_minimum_remote_assets() {
        let fs = ThemeFileSystem {
            root: std::path::PathBuf::new(),
            files: BTreeMap::new(),
            filters: IgnoreFilters::default(),
        };

        let plan = plan_push(
            &fs,
            MINIMUM_THEME_FILES
                .iter()
                .map(|key| Checksum {
                    key: (*key).into(),
                    checksum: "remote".into(),
                })
                .chain(std::iter::once(Checksum {
                    key: "snippets/old.liquid".into(),
                    checksum: "remote".into(),
                }))
                .collect(),
            &SyncOptions::default(),
        );

        assert_eq!(plan.delete, vec!["snippets/old.liquid"]);
    }

    struct RetryApi {
        attempts: Mutex<BTreeMap<String, usize>>,
        batches: Mutex<Vec<Vec<String>>>,
    }

    #[async_trait]
    impl ThemeSyncAdmin for RetryApi {
        async fn fetch_checksums(&self, _theme_id: i64) -> Result<Vec<Checksum>, SyncError> {
            Ok(MINIMUM_THEME_FILES
                .iter()
                .map(|key| Checksum {
                    key: (*key).into(),
                    checksum: "remote".into(),
                })
                .collect())
        }

        async fn fetch_assets(
            &self,
            _theme_id: i64,
            _keys: Vec<String>,
        ) -> Result<Vec<ThemeAsset>, SyncError> {
            Ok(Vec::new())
        }

        async fn upload_assets(
            &self,
            _theme_id: i64,
            assets: Vec<ThemeAsset>,
        ) -> Result<Vec<RemoteResult>, SyncError> {
            self.batches
                .lock()
                .unwrap()
                .push(assets.iter().map(|asset| asset.key.clone()).collect());
            let mut attempts = self.attempts.lock().unwrap();
            Ok(assets
                .into_iter()
                .map(|asset| {
                    let count = attempts.entry(asset.key.clone()).or_insert(0);
                    *count += 1;
                    let success = asset.key != "assets/retry.js" || *count >= 2;
                    RemoteResult {
                        key: asset.key,
                        success,
                        errors: if success {
                            Vec::new()
                        } else {
                            vec!["temporary".into()]
                        },
                    }
                })
                .collect())
        }

        async fn delete_assets(
            &self,
            _theme_id: i64,
            keys: Vec<String>,
        ) -> Result<Vec<RemoteResult>, SyncError> {
            Ok(keys
                .into_iter()
                .map(|key| RemoteResult {
                    key,
                    success: true,
                    errors: Vec::new(),
                })
                .collect())
        }
    }

    #[tokio::test]
    async fn push_retries_only_failed_uploads() {
        let api = RetryApi {
            attempts: Mutex::new(BTreeMap::new()),
            batches: Mutex::new(Vec::new()),
        };
        let mut files = BTreeMap::new();
        files.insert("assets/ok.js".into(), asset("assets/ok.js", 1));
        files.insert("assets/retry.js".into(), asset("assets/retry.js", 1));
        let fs = ThemeFileSystem {
            root: std::path::PathBuf::new(),
            files,
            filters: IgnoreFilters::default(),
        };

        let report = push(&api, 1, &fs, &SyncOptions::default()).await.unwrap();

        assert!(!report.has_failures());
        assert_eq!(
            api.batches.lock().unwrap().clone(),
            vec![
                vec!["assets/ok.js".to_string(), "assets/retry.js".to_string()],
                vec!["assets/retry.js".to_string()]
            ]
        );
    }
}
