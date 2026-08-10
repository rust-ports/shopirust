use crate::error::AppError;
use cli_api::{BundleFormat, DeveloperPlatformClient, MinimalAppIdentifiers};
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

pub const MAX_BUNDLE_SIZE_BYTES: u64 = 100 * 1024 * 1024;
pub const BUNDLE_EXCLUSION_SUFFIXES: &[&str] = &[".js.map", ".metafile.json"];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppManifest {
    pub name: String,
    pub handle: Option<String>,
    pub modules: Vec<serde_json::Value>,
}

pub fn write_manifest_to_bundle(manifest: &AppManifest, bundle_dir: &Path) -> Result<(), AppError> {
    fs::create_dir_all(bundle_dir)?;
    let path = bundle_dir.join("manifest.json");
    let json = serde_json::to_string_pretty(manifest)?;
    fs::write(path, json)?;
    Ok(())
}

/// Compress `input_directory` to `output_path` (`.br` or `.zip`).
pub fn compress_bundle(input_directory: &Path, output_path: &Path) -> Result<(), AppError> {
    if output_path
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e == "br")
    {
        compress_brotli(input_directory, output_path)?;
    } else {
        compress_zip(input_directory, output_path)?;
    }
    let size = fs::metadata(output_path)?.len();
    if size > MAX_BUNDLE_SIZE_BYTES {
        let _ = fs::remove_file(output_path);
        return Err(AppError::message(format!(
            "Bundle exceeds the 100 MB size limit ({} bytes)",
            size
        )));
    }
    Ok(())
}

fn should_exclude(path: &Path) -> bool {
    let name = path.to_string_lossy();
    BUNDLE_EXCLUSION_SUFFIXES.iter().any(|s| name.ends_with(s))
}

fn collect_files(dir: &Path, base: &Path, out: &mut Vec<(PathBuf, PathBuf)>) -> Result<(), AppError> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            collect_files(&path, base, out)?;
        } else if !should_exclude(&path) {
            let rel = path
                .strip_prefix(base)
                .unwrap_or(&path)
                .to_path_buf();
            out.push((path, rel));
        }
    }
    Ok(())
}

fn compress_zip(input_directory: &Path, output_path: &Path) -> Result<(), AppError> {
    let file = fs::File::create(output_path)?;
    let mut zip = zip::ZipWriter::new(file);
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    let mut files = Vec::new();
    collect_files(input_directory, input_directory, &mut files)?;
    for (abs, rel) in files {
        let name = rel.to_string_lossy().replace('\\', "/");
        zip.start_file(name, options)
            .map_err(|e| AppError::message(e.to_string()))?;
        let mut f = fs::File::open(abs)?;
        let mut buf = Vec::new();
        f.read_to_end(&mut buf)?;
        zip.write_all(&buf)?;
    }
    zip.finish()
        .map_err(|e| AppError::message(e.to_string()))?;
    Ok(())
}

fn compress_brotli(input_directory: &Path, output_path: &Path) -> Result<(), AppError> {
    // Produce a zip first in memory/temp then brotli-compress the bytes —
    // upstream stores a brotli-compressed archive of the directory tree.
    let tmp_zip = output_path.with_extension("zip.tmp");
    compress_zip(input_directory, &tmp_zip)?;
    let raw = fs::read(&tmp_zip)?;
    let _ = fs::remove_file(&tmp_zip);
    let mut out = fs::File::create(output_path)?;
    let mut encoder = brotli::CompressorWriter::new(&mut out, 4096, 5, 22);
    encoder
        .write_all(&raw)
        .map_err(|e| AppError::message(e.to_string()))?;
    encoder
        .flush()
        .map_err(|e| AppError::message(e.to_string()))?;
    Ok(())
}

pub async fn get_upload_url(
    client: &dyn DeveloperPlatformClient,
    app: &MinimalAppIdentifiers,
) -> Result<String, AppError> {
    let schema = client
        .generate_signed_upload_url(app)
        .await
        .map_err(|e| AppError::message(e.to_string()))?;
    if !schema.user_errors.is_empty() {
        let msg = schema
            .user_errors
            .into_iter()
            .map(|e| e.message)
            .collect::<Vec<_>>()
            .join("; ");
        return Err(AppError::message(msg));
    }
    schema
        .asset_url
        .ok_or_else(|| AppError::message("No signed upload URL returned"))
}

pub async fn upload_to_gcs(upload_url: &str, bundle_path: &Path) -> Result<(), AppError> {
    let bytes = fs::read(bundle_path)?;
    let client = reqwest::Client::new();
    let content_type = if bundle_path.extension().and_then(|e| e.to_str()) == Some("br") {
        "application/x-brotli"
    } else {
        "application/zip"
    };
    let resp = client
        .put(upload_url)
        .header(reqwest::header::CONTENT_TYPE, content_type)
        .body(bytes)
        .send()
        .await
        .map_err(|e| AppError::message(e.to_string()))?;
    if !resp.status().is_success() {
        return Err(AppError::message(format!(
            "Failed to upload bundle: HTTP {}",
            resp.status()
        )));
    }
    Ok(())
}

pub fn bundle_extension_for_format(format: BundleFormat) -> &'static str {
    match format {
        BundleFormat::Br => "br",
        BundleFormat::Zip => "zip",
    }
}

pub fn default_bundle_path(directory: &Path, format: BundleFormat) -> PathBuf {
    let shopify = directory.join(".shopify");
    let _ = fs::create_dir_all(&shopify);
    shopify.join(format!(
        "deploy-bundle.{}",
        bundle_extension_for_format(format)
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn write_and_compress_zip() {
        let dir = tempdir().unwrap();
        let bundle_dir = dir.path().join("bundle");
        fs::create_dir_all(bundle_dir.join("ext")).unwrap();
        fs::write(bundle_dir.join("ext/file.txt"), "hello").unwrap();
        write_manifest_to_bundle(
            &AppManifest {
                name: "Demo".into(),
                handle: None,
                modules: vec![],
            },
            &bundle_dir,
        )
        .unwrap();
        let out = dir.path().join("out.zip");
        compress_bundle(&bundle_dir, &out).unwrap();
        assert!(out.exists());
        assert!(fs::metadata(&out).unwrap().len() > 0);
    }

    #[test]
    fn excludes_sourcemaps() {
        let dir = tempdir().unwrap();
        let bundle_dir = dir.path().join("bundle");
        fs::create_dir_all(&bundle_dir).unwrap();
        fs::write(bundle_dir.join("keep.js"), "x").unwrap();
        fs::write(bundle_dir.join("skip.js.map"), "y").unwrap();
        let out = dir.path().join("out.zip");
        compress_bundle(&bundle_dir, &out).unwrap();
        let file = fs::File::open(&out).unwrap();
        let mut archive = zip::ZipArchive::new(file).unwrap();
        let names: Vec<String> = (0..archive.len())
            .map(|i| archive.by_index(i).unwrap().name().to_string())
            .collect();
        assert!(names.iter().any(|n| n.contains("keep.js")));
        assert!(!names.iter().any(|n| n.contains("skip.js.map")));
    }
}
