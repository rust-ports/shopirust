use std::collections::HashMap;
use std::fs;
use std::path::Path;

struct FrameworkDetectionPattern {
    path: String,
    match_content: Option<String>,
}

struct Framework {
    name: &'static str,
    detectors: FrameworkDetectors,
}

struct FrameworkDetectors {
    every: Option<Vec<FrameworkDetectionPattern>>,
    some: Option<Vec<FrameworkDetectionPattern>>,
}

static FRAMEWORKS: once_cell::sync::Lazy<Vec<Framework>> = once_cell::sync::Lazy::new(|| {
    fn make_pattern(path: &str, content: Option<&str>) -> FrameworkDetectionPattern {
        FrameworkDetectionPattern {
            path: path.to_string(),
            match_content: content.map(|c| c.to_string()),
        }
    }

    fn make_fw(
        name: &'static str,
        every: Option<Vec<FrameworkDetectionPattern>>,
        some: Option<Vec<FrameworkDetectionPattern>>,
    ) -> Framework {
        Framework {
            name,
            detectors: FrameworkDetectors { every, some },
        }
    }

    vec![
        make_fw(
            "remix",
            Some(vec![
                make_pattern(
                    "package.json",
                    Some(r#""(dev)?(d|D)ependencies":\s*\{[^}]*"@remix-run\/.*":\s*".+?"[^}]*\}"#),
                ),
                make_pattern(
                    "package.json",
                    Some(r#""(dev)?(d|D)ependencies":\s*\{[^}]*"react":\s*".+?"[^}]*\}"#),
                ),
            ]),
            None,
        ),
        make_fw(
            "nextjs",
            Some(vec![
                make_pattern(
                    "package.json",
                    Some(r#""(dev)?(d|D)ependencies":\s*\{[^}]*"next":\s*".+?"[^}]*\}"#),
                ),
                make_pattern(
                    "package.json",
                    Some(r#""(dev)?(d|D)ependencies":\s*\{[^}]*"react":\s*".+?"[^}]*\}"#),
                ),
            ]),
            None,
        ),
        make_fw(
            "express",
            Some(vec![make_pattern(
                "package.json",
                Some(r#""(dev)?(d|D)ependencies":\s*\{[^}]*"express":\s*".+?"[^}]*\}"#),
            )]),
            None,
        ),
        make_fw(
            "rails",
            Some(vec![make_pattern("Gemfile", Some(r#"gem "rails""#))]),
            None,
        ),
        make_fw(
            "flask",
            Some(vec![make_pattern("Pipfile", Some("flask"))]),
            None,
        ),
        make_fw(
            "django",
            Some(vec![make_pattern("Pipfile", Some("django"))]),
            None,
        ),
        make_fw(
            "laravel",
            Some(vec![make_pattern(
                "composer.json",
                Some(r#""require":\s*\{[^}]*"laravel/framework":\s*".+?"[^}]*\}"#),
            )]),
            None,
        ),
        make_fw(
            "symfony",
            Some(vec![make_pattern(
                "composer.json",
                Some(r#""require":\s*\{[^}]*"symfony\/.*":\s*".+?"[^}]*\}"#),
            )]),
            None,
        ),
    ]
});

pub fn resolve_framework(root_directory: &str) -> String {
    let mut fw_config_files: HashMap<String, String> = HashMap::new();

    for framework in FRAMEWORKS.iter() {
        let every_match = match &framework.detectors.every {
            Some(detectors) => detectors.iter().all(|d| {
                load_fw_config_file(root_directory, &d.path, &mut fw_config_files);
                match_detector(d, &fw_config_files)
            }),
            None => true,
        };

        if !every_match {
            continue;
        }

        let some_match = match &framework.detectors.some {
            Some(detectors) => detectors.iter().any(|d| {
                load_fw_config_file(root_directory, &d.path, &mut fw_config_files);
                match_detector(d, &fw_config_files)
            }),
            None => true,
        };

        if some_match {
            return framework.name.to_string();
        }
    }

    "unknown".to_string()
}

fn match_detector(
    detector: &FrameworkDetectionPattern,
    fw_config_files: &HashMap<String, String>,
) -> bool {
    let content = match fw_config_files.get(&detector.path) {
        Some(c) => c,
        None => return false,
    };

    match &detector.match_content {
        Some(pattern) => regex_lite::Regex::new(pattern)
            .ok()
            .is_some_and(|re| re.is_match(content)),
        None => true,
    }
}

fn load_fw_config_file(
    root_path: &str,
    fw_config_file_name: &str,
    fw_config_files: &mut HashMap<String, String>,
) {
    if fw_config_files.contains_key(fw_config_file_name) {
        return;
    }

    let fw_config_path = Path::new(root_path).join(fw_config_file_name);
    if !fw_config_path.exists() {
        return;
    }

    if let Ok(content) = fs::read_to_string(&fw_config_path) {
        fw_config_files.insert(fw_config_file_name.to_string(), content);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::io::Write;
    use tempfile::tempdir;

    #[test]
    fn test_resolve_framework_unknown() {
        let dir = tempdir().unwrap();
        let result = resolve_framework(dir.path().to_str().unwrap());
        assert_eq!(result, "unknown");
    }

    #[test]
    fn test_resolve_framework_express() {
        let dir = tempdir().unwrap();
        let pkg_path = dir.path().join("package.json");
        let mut file = fs::File::create(&pkg_path).unwrap();
        file.write_all(b"{\"dependencies\":{\"express\":\"^4.18.0\"}}")
            .unwrap();
        drop(file);

        let result = resolve_framework(dir.path().to_str().unwrap());
        assert_eq!(result, "express");
    }

    #[test]
    fn test_resolve_framework_rails() {
        let dir = tempdir().unwrap();
        let gemfile_path = dir.path().join("Gemfile");
        let mut file = fs::File::create(&gemfile_path).unwrap();
        writeln!(file, r#"gem "rails""#).unwrap();
        drop(file);

        let result = resolve_framework(dir.path().to_str().unwrap());
        assert_eq!(result, "rails");
    }

    #[test]
    fn test_resolve_framework_remix() {
        let dir = tempdir().unwrap();
        let pkg_path = dir.path().join("package.json");
        let mut file = fs::File::create(&pkg_path).unwrap();
        file.write_all(
            b"{\"dependencies\":{\"@remix-run/react\":\"^2.0.0\",\"react\":\"^18.0.0\"}}",
        )
        .unwrap();
        drop(file);

        let result = resolve_framework(dir.path().to_str().unwrap());
        assert_eq!(result, "remix");
    }

    #[test]
    fn test_resolve_framework_nextjs() {
        let dir = tempdir().unwrap();
        let pkg_path = dir.path().join("package.json");
        let mut file = fs::File::create(&pkg_path).unwrap();
        file.write_all(b"{\"dependencies\":{\"next\":\"^14.0.0\",\"react\":\"^18.0.0\"}}")
            .unwrap();
        drop(file);

        let result = resolve_framework(dir.path().to_str().unwrap());
        assert_eq!(result, "nextjs");
    }
}
