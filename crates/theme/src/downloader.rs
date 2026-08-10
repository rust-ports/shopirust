use crate::filesystem::ThemeFileSystem;
use crate::sync::{
    self, FileOperation, FileOperationReport, SyncError, SyncOptions, SyncReport, ThemeSyncAdmin,
};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ThemeDownloadReport {
    pub sync: SyncReport,
    pub downloaded: BTreeMap<String, FileOperationReport>,
    pub deleted: BTreeMap<String, FileOperationReport>,
    pub failed_downloads: BTreeMap<String, Vec<String>>,
}

impl ThemeDownloadReport {
    pub fn has_failures(&self) -> bool {
        self.sync.has_failures()
    }
}

pub async fn download_theme<A: ThemeSyncAdmin + Sync>(
    api: &A,
    theme_id: i64,
    filesystem: &mut ThemeFileSystem,
    options: &SyncOptions,
) -> Result<ThemeDownloadReport, SyncError> {
    let sync = sync::pull(api, theme_id, filesystem, options).await?;
    Ok(report_from_sync(sync))
}

pub fn report_from_sync(sync: SyncReport) -> ThemeDownloadReport {
    let mut report = ThemeDownloadReport {
        sync,
        ..ThemeDownloadReport::default()
    };

    for file in &report.sync.files {
        match file.operation {
            FileOperation::Download if file.success => {
                report.downloaded.insert(file.key.clone(), file.clone());
            }
            FileOperation::Download => {
                report
                    .failed_downloads
                    .insert(file.key.clone(), file.errors.clone());
            }
            FileOperation::Delete => {
                report.deleted.insert(file.key.clone(), file.clone());
            }
            FileOperation::Upload => {}
        }
    }

    report
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_download_result_maps() {
        let report = report_from_sync(SyncReport {
            files: vec![
                FileOperationReport {
                    key: "templates/index.json".into(),
                    operation: FileOperation::Download,
                    success: true,
                    errors: vec![],
                },
                FileOperationReport {
                    key: "sections/missing.liquid".into(),
                    operation: FileOperation::Download,
                    success: false,
                    errors: vec!["The remote file was not returned".into()],
                },
                FileOperationReport {
                    key: "assets/local-only.js".into(),
                    operation: FileOperation::Delete,
                    success: true,
                    errors: vec![],
                },
            ],
        });

        assert!(report.has_failures());
        assert!(report.downloaded.contains_key("templates/index.json"));
        assert_eq!(
            report.failed_downloads["sections/missing.liquid"],
            vec!["The remote file was not returned"]
        );
        assert!(report.deleted.contains_key("assets/local-only.js"));
    }
}
