use regex_lite::Regex;
use std::collections::HashSet;
use std::fs;
use std::io;
use std::path::Path;

pub const SHOPIFY_IGNORE: &str = ".shopifyignore";

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct IgnoreFilters {
    pub ignore_from_file: Vec<String>,
    pub ignore: Vec<String>,
    pub only: Vec<String>,
}

pub trait ThemeFileKey {
    fn key(&self) -> &str;
}

impl ThemeFileKey for String {
    fn key(&self) -> &str {
        self
    }
}

impl ThemeFileKey for &str {
    fn key(&self) -> &str {
        self
    }
}

pub fn apply_ignore_filters<T>(files: Vec<T>, filters: &IgnoreFilters) -> Vec<T>
where
    T: ThemeFileKey + Clone,
{
    let (normal_shopify, negated_shopify) = split_negated_patterns(&filters.ignore_from_file);
    let (normal_ignore, negated_ignore) = split_negated_patterns(&filters.ignore);
    let (normal_only, negated_only) = split_negated_patterns(&filters.only);

    let mut filtered = filter_files(&files, &normal_shopify, false);
    filtered = filter_files(&filtered, &normal_ignore, false);
    filtered = filter_files(&filtered, &normal_only, true);

    if !negated_shopify.is_empty() {
        filtered.extend(filter_files(&files, &negated_shopify, true));
    }

    if !negated_ignore.is_empty() {
        filtered.extend(filter_files(&files, &negated_ignore, true));
    }

    if !negated_only.is_empty() {
        filtered = filter_files(&filtered, &negated_only, false);
    }

    unique_by_key(filtered)
}

pub fn get_patterns_from_shopify_ignore(root: impl AsRef<Path>) -> io::Result<Vec<String>> {
    let path = root.as_ref().join(SHOPIFY_IGNORE);
    if !path.exists() {
        return Ok(Vec::new());
    }

    let content = fs::read_to_string(path)?;
    Ok(content
        .split(['\r', '\n'])
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(ToOwned::to_owned)
        .collect())
}

pub fn is_match(key: &str, pattern: &str) -> bool {
    match_glob(key, pattern)
        || (is_regex(pattern) && regex_match(key, &pattern[1..pattern.len() - 1]))
}

fn split_negated_patterns(patterns: &[String]) -> (Vec<&str>, Vec<&str>) {
    let mut normal = Vec::new();
    let mut negated = Vec::new();

    for pattern in patterns {
        if let Some(stripped) = pattern.strip_prefix('!') {
            negated.push(stripped);
        } else {
            normal.push(pattern.as_str());
        }
    }

    (normal, negated)
}

fn filter_files<T>(files: &[T], patterns: &[&str], invert_match: bool) -> Vec<T>
where
    T: ThemeFileKey + Clone,
{
    if patterns.is_empty() {
        return files.to_vec();
    }

    files
        .iter()
        .filter(|file| {
            let matches = patterns.iter().any(|pattern| is_match(file.key(), pattern));
            let should_ignore = if invert_match { !matches } else { matches };
            !should_ignore
        })
        .cloned()
        .collect()
}

fn unique_by_key<T>(files: Vec<T>) -> Vec<T>
where
    T: ThemeFileKey,
{
    let mut seen = HashSet::new();
    let mut unique = Vec::new();

    for file in files {
        if seen.insert(file.key().to_string()) {
            unique.push(file);
        }
    }

    unique
}

fn match_glob(key: &str, pattern: &str) -> bool {
    if glob_match(key, pattern) {
        return true;
    }

    if let Some(extension) = template_compat_extension(pattern) {
        return key.starts_with("templates/")
            && extension.map_or(true, |extension| key.ends_with(extension));
    }

    false
}

fn template_compat_extension(pattern: &str) -> Option<Option<&str>> {
    match pattern {
        "templates/*" => Some(None),
        "templates/*.json" => Some(Some(".json")),
        "templates/*.liquid" => Some(Some(".liquid")),
        _ => None,
    }
}

fn glob_match(key: &str, pattern: &str) -> bool {
    if pattern.is_empty() {
        return key.is_empty();
    }

    if pattern.ends_with('/') {
        return directory_pattern_match(key, pattern);
    }

    let target = if pattern.contains('/') {
        key
    } else {
        key.rsplit('/').next().unwrap_or(key)
    };

    Regex::new(&glob_regex(pattern))
        .map(|regex| regex.is_match(target))
        .unwrap_or(false)
}

fn directory_pattern_match(key: &str, pattern: &str) -> bool {
    let directory = pattern.trim_end_matches('/');
    if directory.is_empty() {
        return false;
    }

    if pattern.contains('/') {
        key == directory || key.starts_with(&format!("{directory}/"))
    } else {
        key.split('/').any(|part| part == directory)
    }
}

fn glob_regex(pattern: &str) -> String {
    let mut regex = String::from("^");
    let mut chars = pattern.chars().peekable();

    while let Some(ch) = chars.next() {
        match ch {
            '*' if chars.peek() == Some(&'*') => {
                chars.next();
                if chars.peek() == Some(&'/') {
                    chars.next();
                    regex.push_str("(?:.*/)?");
                } else {
                    regex.push_str(".*");
                }
            }
            '*' => regex.push_str("[^/]*"),
            '?' => regex.push_str("[^/]"),
            '{' => {
                let mut options = String::new();
                for next in chars.by_ref() {
                    if next == '}' {
                        break;
                    }
                    options.push(next);
                }
                if options.is_empty() {
                    regex.push_str("\\{\\}");
                } else {
                    regex.push('(');
                    regex.push_str(
                        &options
                            .split(',')
                            .map(regex_escape)
                            .collect::<Vec<_>>()
                            .join("|"),
                    );
                    regex.push(')');
                }
            }
            '.' | '+' | '(' | ')' | '|' | '^' | '$' | '[' | ']' | '\\' => {
                regex.push('\\');
                regex.push(ch);
            }
            _ => regex.push(ch),
        }
    }

    regex.push('$');
    regex
}

fn regex_escape(value: &str) -> String {
    let mut output = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '.' | '+' | '(' | ')' | '|' | '^' | '$' | '[' | ']' | '\\' | '*' | '?' | '{' | '}' => {
                output.push('\\');
                output.push(ch);
            }
            _ => output.push(ch),
        }
    }
    output
}

fn is_regex(pattern: &str) -> bool {
    pattern.starts_with('/') && pattern.ends_with('/') && pattern.len() >= 2
}

fn regex_match(key: &str, pattern: &str) -> bool {
    Regex::new(pattern)
        .map(|regex| regex.is_match(key))
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[derive(Clone)]
    struct File {
        key: String,
    }

    impl ThemeFileKey for File {
        fn key(&self) -> &str {
            &self.key
        }
    }

    fn keys(files: Vec<File>) -> Vec<String> {
        files.into_iter().map(|file| file.key).collect()
    }

    fn files(keys: &[&str]) -> Vec<File> {
        keys.iter()
            .map(|key| File {
                key: (*key).to_string(),
            })
            .collect()
    }

    #[test]
    fn applies_ignore_and_only_filters_in_upstream_order() {
        let result = apply_ignore_filters(
            files(&["assets/a.css", "assets/b.css", "templates/index.json"]),
            &IgnoreFilters {
                ignore: vec!["*.css".into()],
                only: vec!["templates/*".into()],
                ..IgnoreFilters::default()
            },
        );

        assert_eq!(keys(result), vec!["templates/index.json"]);
    }

    #[test]
    fn supports_negated_ignore_patterns() {
        let result = apply_ignore_filters(
            files(&["assets/a.css", "assets/keep.css", "templates/index.json"]),
            &IgnoreFilters {
                ignore: vec!["assets/*".into(), "!assets/keep.css".into()],
                ..IgnoreFilters::default()
            },
        );

        assert_eq!(
            keys(result),
            vec!["templates/index.json", "assets/keep.css"]
        );
    }

    #[test]
    fn supports_negated_only_patterns() {
        let result = apply_ignore_filters(
            files(&["assets/a.css", "assets/keep.css", "templates/index.json"]),
            &IgnoreFilters {
                only: vec!["assets/*".into(), "!assets/a.css".into()],
                ..IgnoreFilters::default()
            },
        );

        assert_eq!(keys(result), vec!["assets/keep.css"]);
    }

    #[test]
    fn supports_regex_patterns() {
        let result = apply_ignore_filters(
            files(&["assets/a.css", "assets/a.js", "templates/index.json"]),
            &IgnoreFilters {
                ignore: vec![r"/assets\/.*\.css/".into()],
                ..IgnoreFilters::default()
            },
        );

        assert_eq!(keys(result), vec!["assets/a.js", "templates/index.json"]);
    }

    #[test]
    fn supports_templates_glob_compatibility() {
        assert!(is_match(
            "templates/customers/account.json",
            "templates/*.json"
        ));
    }

    #[test]
    fn reads_shopify_ignore_patterns() {
        let temp = tempfile::tempdir().unwrap();
        fs::write(
            temp.path().join(SHOPIFY_IGNORE),
            "# comment\r\n\r\nassets/*\n !assets/keep.css \n",
        )
        .unwrap();

        assert_eq!(
            get_patterns_from_shopify_ignore(temp.path()).unwrap(),
            vec!["assets/*", "!assets/keep.css"]
        );
    }
}
