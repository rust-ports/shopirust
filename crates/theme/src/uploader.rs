use crate::filesystem::ThemeFileSystem;
use crate::sync::{
    self, FileOperation, FileOperationReport, SyncError, SyncOptions, SyncReport, ThemeSyncAdmin,
};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ThemeUploadReport {
    pub sync: SyncReport,
    pub uploaded: BTreeMap<String, FileOperationReport>,
    pub deleted: BTreeMap<String, FileOperationReport>,
    pub failed_uploads: BTreeMap<String, Vec<String>>,
}

impl ThemeUploadReport {
    pub fn has_failures(&self) -> bool {
        self.sync.has_failures()
    }
}

pub async fn upload_theme<A: ThemeSyncAdmin + Sync>(
    api: &A,
    theme_id: i64,
    filesystem: &ThemeFileSystem,
    options: &SyncOptions,
) -> Result<ThemeUploadReport, SyncError> {
    let sync = sync::push(api, theme_id, filesystem, options).await?;
    Ok(report_from_sync(sync))
}

pub fn report_from_sync(sync: SyncReport) -> ThemeUploadReport {
    let mut report = ThemeUploadReport {
        sync,
        ..ThemeUploadReport::default()
    };

    for file in &report.sync.files {
        match file.operation {
            FileOperation::Upload if file.success => {
                report.uploaded.insert(file.key.clone(), file.clone());
            }
            FileOperation::Upload => {
                report
                    .failed_uploads
                    .insert(file.key.clone(), file.errors.clone());
            }
            FileOperation::Delete => {
                report.deleted.insert(file.key.clone(), file.clone());
            }
            FileOperation::Download => {}
        }
    }

    report
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_upload_result_maps() {
        let report = report_from_sync(SyncReport {
            files: vec![
                FileOperationReport {
                    key: "assets/app.js".into(),
                    operation: FileOperation::Upload,
                    success: true,
                    errors: vec![],
                },
                FileOperationReport {
                    key: "snippets/card.liquid".into(),
                    operation: FileOperation::Upload,
                    success: false,
                    errors: vec!["invalid liquid".into()],
                },
                FileOperationReport {
                    key: "assets/old.js".into(),
                    operation: FileOperation::Delete,
                    success: true,
                    errors: vec![],
                },
            ],
        });

        assert!(report.has_failures());
        assert!(report.uploaded.contains_key("assets/app.js"));
        assert_eq!(
            report.failed_uploads["snippets/card.liquid"],
            vec!["invalid liquid"]
        );
        assert!(report.deleted.contains_key("assets/old.js"));
    }
}
