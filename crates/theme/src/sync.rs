use crate::checksum::{reject_generated_static_assets, Checksum};
use crate::filesystem::{ThemeAsset, ThemeFileSystem, ThemeFsError};
use crate::ignore::{apply_ignore_filters, IgnoreFilters, ThemeFileKey};
use async_trait::async_trait;
use std::collections::{BTreeMap, BTreeSet};

pub const DOWNLOAD_BATCH_SIZE: usize = 50;
pub const MUTATION_BATCH_SIZE: usize = 20;
pub const UPLOAD_BATCH_BYTES: usize = 1024 * 1024;
pub const MAX_UPLOAD_ATTEMPTS: usize = 3;
pub const MINIMUM_THEME_ASSETS: [(&str, &str); 3] = [
    ("config/settings_schema.json", "[]"),
    (
        "layout/password.liquid",
        "{{ content_for_header }}{{ content_for_layout }}",
    ),
    (
        "layout/theme.liquid",
        "{{ content_for_header }}{{ content_for_layout }}",
    ),
];
pub const MINIMUM_THEME_FILES: [&str; 3] = [
    "config/settings_schema.json",
    "layout/password.liquid",
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
    for (key, value) in MINIMUM_THEME_ASSETS {
        if !remote_by_key.contains_key(key) && !local_files.contains_key(key) {
            let value = value.to_string();
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
                !is_minimum_theme_file(&item.key) && !local_files.contains_key(&item.key)
            })
            .map(|item| item.key)
            .collect()
    };
    PushPlan { upload, delete }
}

fn is_minimum_theme_file(key: &str) -> bool {
    MINIMUM_THEME_FILES.contains(&key)
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

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct UploadWork {
    pub independent: Vec<ThemeAsset>,
    pub dependent: Vec<Vec<ThemeAsset>>,
}

pub fn ordered_upload_work(assets: Vec<ThemeAsset>) -> UploadWork {
    let mut independent = Vec::new();
    let mut dependent: BTreeMap<AssetGroup, Vec<ThemeAsset>> = BTreeMap::new();
    for asset in assets {
        let group = classify(&asset.key);
        if group == AssetGroup::Independent {
            independent.push(asset);
        } else {
            dependent.entry(group).or_default().push(asset);
        }
    }
    UploadWork {
        independent,
        dependent: dependent.into_values().collect(),
    }
}

pub fn ordered_deletions(mut keys: Vec<String>) -> Vec<String> {
    keys.sort_by_key(|key| (classify_deletion(key), key.clone()));
    keys
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum DeleteGroup {
    ContextualJson,
    JsonTemplate,
    JsonSection,
    OtherJson,
    LiquidSection,
    Block,
    Layout,
    OtherLiquid,
    SettingsData,
    SettingsSchema,
    StaticAsset,
}

fn classify_deletion(key: &str) -> DeleteGroup {
    if key.starts_with("templates/") && key.ends_with(".json") && key.contains(".context.") {
        DeleteGroup::ContextualJson
    } else if key.starts_with("templates/") && key.ends_with(".json") {
        DeleteGroup::JsonTemplate
    } else if key.starts_with("sections/") && key.ends_with(".json") {
        DeleteGroup::JsonSection
    } else if key.ends_with(".json")
        && key != "config/settings_schema.json"
        && key != "config/settings_data.json"
    {
        DeleteGroup::OtherJson
    } else if key.starts_with("sections/") && key.ends_with(".liquid") {
        DeleteGroup::LiquidSection
    } else if key.starts_with("blocks/") && key.ends_with(".liquid") {
        DeleteGroup::Block
    } else if key.starts_with("layout/") && key.ends_with(".liquid") {
        DeleteGroup::Layout
    } else if key.ends_with(".liquid") {
        DeleteGroup::OtherLiquid
    } else if key == "config/settings_data.json" {
        DeleteGroup::SettingsData
    } else if key == "config/settings_schema.json" {
        DeleteGroup::SettingsSchema
    } else {
        DeleteGroup::StaticAsset
    }
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

    let upload_work = ordered_upload_work(plan.upload);
    let upload_independent = async {
        let mut files = Vec::new();
        for batch in upload_batches(&upload_work.independent) {
            files.extend(reconcile_upload(api, theme_id, batch).await?);
        }
        Ok::<_, SyncError>(files)
    };
    let upload_dependent = async {
        let mut files = Vec::new();
        for group in upload_work.dependent {
            for batch in upload_batches(&group) {
                files.extend(reconcile_upload(api, theme_id, batch).await?);
            }
        }
        Ok::<_, SyncError>(files)
    };
    let (independent_files, dependent_files) =
        futures::try_join!(upload_independent, upload_dependent)?;
    report.files.extend(independent_files);
    report.files.extend(dependent_files);

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
) -> Result<Vec<FileOperationReport>, SyncError> {
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
    Ok(final_results
        .into_values()
        .map(|result| from_remote(result, FileOperation::Upload))
        .collect())
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
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Mutex;
    use std::time::Duration;

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
    fn upload_work_splits_independent_files_from_dependent_chain() {
        let work = ordered_upload_work(vec![
            asset("config/settings_data.json", 1),
            asset("layout/theme.liquid", 1),
            asset("assets/a.js", 1),
            asset("locales/en.default.json", 1),
            asset("sections/header.liquid", 1),
            asset("snippets/card.liquid", 1),
        ]);

        assert_eq!(
            work.independent
                .iter()
                .map(|asset| asset.key.as_str())
                .collect::<Vec<_>>(),
            vec![
                "assets/a.js",
                "locales/en.default.json",
                "snippets/card.liquid"
            ]
        );
        assert_eq!(
            work.dependent
                .iter()
                .map(|group| classify(&group[0].key))
                .collect::<Vec<_>>(),
            vec![
                AssetGroup::Layout,
                AssetGroup::LiquidSection,
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
        assert_eq!(
            plan.upload
                .iter()
                .map(|asset| (
                    asset.key.as_str(),
                    asset.value.as_deref().unwrap_or_default()
                ))
                .collect::<Vec<_>>(),
            MINIMUM_THEME_ASSETS
        );
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

    #[test]
    fn deletions_match_upstream_dependency_order() {
        let keys = vec![
            "assets/a.css",
            "config/settings_schema.json",
            "config/settings_data.json",
            "layout/theme.liquid",
            "snippets/card.liquid",
            "blocks/card.liquid",
            "sections/main.liquid",
            "config/markets.json",
            "sections/header.json",
            "templates/product.json",
            "templates/product.context.us.json",
        ]
        .into_iter()
        .map(String::from)
        .collect();

        assert_eq!(
            ordered_deletions(keys),
            vec![
                "templates/product.context.us.json",
                "templates/product.json",
                "sections/header.json",
                "config/markets.json",
                "sections/main.liquid",
                "blocks/card.liquid",
                "layout/theme.liquid",
                "snippets/card.liquid",
                "config/settings_data.json",
                "config/settings_schema.json",
                "assets/a.css",
            ]
        );
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

    struct ConcurrentUploadApi {
        independent_done: AtomicBool,
        dependent_started_before_independent_done: AtomicBool,
    }

    #[async_trait]
    impl ThemeSyncAdmin for ConcurrentUploadApi {
        async fn fetch_checksums(&self, _theme_id: i64) -> Result<Vec<Checksum>, SyncError> {
            Ok(Vec::new())
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
            if assets.iter().any(|asset| asset.key == "assets/slow.js") {
                tokio::time::sleep(Duration::from_millis(50)).await;
                self.independent_done.store(true, Ordering::SeqCst);
            }
            if assets
                .iter()
                .any(|asset| asset.key == "config/settings_schema.json")
                && !self.independent_done.load(Ordering::SeqCst)
            {
                self.dependent_started_before_independent_done
                    .store(true, Ordering::SeqCst);
            }
            Ok(assets
                .into_iter()
                .map(|asset| RemoteResult {
                    key: asset.key,
                    success: true,
                    errors: Vec::new(),
                })
                .collect())
        }

        async fn delete_assets(
            &self,
            _theme_id: i64,
            _keys: Vec<String>,
        ) -> Result<Vec<RemoteResult>, SyncError> {
            Ok(Vec::new())
        }
    }

    #[tokio::test]
    async fn push_uploads_independent_files_concurrently_with_dependent_chain() {
        let api = ConcurrentUploadApi {
            independent_done: AtomicBool::new(false),
            dependent_started_before_independent_done: AtomicBool::new(false),
        };
        let mut files = BTreeMap::new();
        files.insert("assets/slow.js".into(), asset("assets/slow.js", 1));
        let fs = ThemeFileSystem {
            root: std::path::PathBuf::new(),
            files,
            filters: IgnoreFilters::default(),
        };

        push(&api, 1, &fs, &SyncOptions::default()).await.unwrap();

        assert!(api
            .dependent_started_before_independent_done
            .load(Ordering::SeqCst));
    }

    fn fs_with(files: BTreeMap<String, ThemeAsset>) -> ThemeFileSystem {
        ThemeFileSystem {
            root: std::path::PathBuf::new(),
            files,
            filters: IgnoreFilters::default(),
        }
    }

    fn asset_with_checksum(key: &str, checksum: &str) -> ThemeAsset {
        ThemeAsset {
            key: key.into(),
            checksum: checksum.into(),
            value: Some("content".into()),
            attachment: None,
            stats: None,
        }
    }

    #[test]
    fn plan_pull_deletes_local_only_files() {
        let mut files = BTreeMap::new();
        files.insert(
            "assets/keepme.css".into(),
            asset_with_checksum("assets/keepme.css", "1"),
        );
        files.insert(
            "assets/deleteme.css".into(),
            asset_with_checksum("assets/deleteme.css", "2"),
        );
        let plan = plan_pull(
            &fs_with(files),
            vec![Checksum {
                key: "assets/keepme.css".into(),
                checksum: "1".into(),
            }],
            &SyncOptions::default(),
        );
        assert!(plan.download.is_empty());
        assert_eq!(plan.delete, vec!["assets/deleteme.css".to_string()]);
    }

    #[test]
    fn plan_pull_skips_delete_with_nodelete() {
        let mut files = BTreeMap::new();
        files.insert(
            "assets/keepme.css".into(),
            asset_with_checksum("assets/keepme.css", "1"),
        );
        let plan = plan_pull(
            &fs_with(files),
            Vec::new(),
            &SyncOptions {
                nodelete: true,
                ..Default::default()
            },
        );
        assert!(plan.delete.is_empty());
    }

    #[test]
    fn plan_pull_skips_delete_when_only_filter_excludes_local_file() {
        let mut files = BTreeMap::new();
        files.insert(
            "assets/keepme.css".into(),
            asset_with_checksum("assets/keepme.css", "1"),
        );
        let plan = plan_pull(
            &fs_with(files),
            Vec::new(),
            &SyncOptions {
                filters: IgnoreFilters {
                    only: vec!["templates/*".into()],
                    ..Default::default()
                },
                ..Default::default()
            },
        );
        assert!(plan.delete.is_empty());
    }

    #[test]
    fn plan_pull_downloads_missing_and_mismatched_remote_files() {
        let mut files = BTreeMap::new();
        files.insert(
            "release/alreadyexists".into(),
            asset_with_checksum("release/alreadyexists", "2"),
        );
        files.insert(
            "release/changed".into(),
            asset_with_checksum("release/changed", "old"),
        );
        let plan = plan_pull(
            &fs_with(files),
            vec![
                Checksum {
                    key: "release/downloadme".into(),
                    checksum: "1".into(),
                },
                Checksum {
                    key: "release/alreadyexists".into(),
                    checksum: "2".into(),
                },
                Checksum {
                    key: "release/changed".into(),
                    checksum: "9".into(),
                },
                Checksum {
                    key: "ignoreme".into(),
                    checksum: "3".into(),
                },
                Checksum {
                    key: "release/ignoreme".into(),
                    checksum: "4".into(),
                },
            ],
            &SyncOptions {
                filters: IgnoreFilters {
                    only: vec!["/release/".into()],
                    ignore: vec!["release/ignoreme".into()],
                    ..Default::default()
                },
                ..Default::default()
            },
        );
        let mut download = plan.download;
        download.sort();
        assert_eq!(
            download,
            vec![
                "release/changed".to_string(),
                "release/downloadme".to_string()
            ]
        );
    }

    #[test]
    fn plan_push_deletes_remote_only_files() {
        let mut files = BTreeMap::new();
        files.insert(
            "assets/keepme.liquid".into(),
            asset_with_checksum("assets/keepme.liquid", "1"),
        );
        let plan = plan_push(
            &fs_with(files),
            vec![
                Checksum {
                    key: "assets/keepme.liquid".into(),
                    checksum: "1".into(),
                },
                Checksum {
                    key: "assets/deleteme.liquid".into(),
                    checksum: "2".into(),
                },
            ],
            &SyncOptions::default(),
        );
        assert!(plan
            .upload
            .iter()
            .all(|asset| asset.key != "assets/keepme.liquid"));
        assert_eq!(plan.delete, vec!["assets/deleteme.liquid".to_string()]);
    }

    #[test]
    fn plan_push_skips_delete_with_nodelete() {
        let mut files = BTreeMap::new();
        files.insert(
            "assets/keepme.liquid".into(),
            asset_with_checksum("assets/keepme.liquid", "1"),
        );
        let plan = plan_push(
            &fs_with(files),
            vec![Checksum {
                key: "assets/deleteme.liquid".into(),
                checksum: "2".into(),
            }],
            &SyncOptions {
                nodelete: true,
                ..Default::default()
            },
        );
        assert!(plan.delete.is_empty());
    }

    #[test]
    fn plan_push_uploads_checksum_mismatches_and_missing() {
        let mut files = BTreeMap::new();
        files.insert(
            "assets/same.css".into(),
            asset_with_checksum("assets/same.css", "1"),
        );
        files.insert(
            "assets/changed.css".into(),
            asset_with_checksum("assets/changed.css", "2"),
        );
        files.insert(
            "assets/new.css".into(),
            asset_with_checksum("assets/new.css", "3"),
        );
        let plan = plan_push(
            &fs_with(files),
            vec![
                Checksum {
                    key: "assets/same.css".into(),
                    checksum: "1".into(),
                },
                Checksum {
                    key: "assets/changed.css".into(),
                    checksum: "old".into(),
                },
            ],
            &SyncOptions::default(),
        );
        let mut keys: Vec<_> = plan.upload.iter().map(|asset| asset.key.clone()).collect();
        keys.sort();
        assert!(keys.contains(&"assets/changed.css".to_string()));
        assert!(keys.contains(&"assets/new.css".to_string()));
        assert!(!keys.contains(&"assets/same.css".to_string()));
    }

    #[test]
    fn plan_push_respects_only_and_ignore_filters() {
        let mut files = BTreeMap::new();
        files.insert(
            "assets/a.css".into(),
            asset_with_checksum("assets/a.css", "1"),
        );
        files.insert(
            "templates/index.json".into(),
            asset_with_checksum("templates/index.json", "1"),
        );
        files.insert(
            "templates/product.json".into(),
            asset_with_checksum("templates/product.json", "1"),
        );
        let plan = plan_push(
            &fs_with(files),
            Vec::new(),
            &SyncOptions {
                filters: IgnoreFilters {
                    only: vec!["templates/*".into()],
                    ignore: vec!["templates/product.json".into()],
                    ..Default::default()
                },
                ..Default::default()
            },
        );
        let keys: Vec<_> = plan
            .upload
            .iter()
            .map(|asset| asset.key.clone())
            .filter(|key| key.starts_with("templates/") || key.starts_with("assets/"))
            .collect();
        assert_eq!(keys, vec!["templates/index.json".to_string()]);
    }
}
