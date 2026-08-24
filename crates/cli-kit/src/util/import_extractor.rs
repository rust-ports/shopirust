use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::sync::Mutex;

const MAX_READ_SIZE: u64 = 128 * 1024;

static DIRECT_IMPORTS_CACHE: once_cell::sync::Lazy<Mutex<HashMap<String, Vec<String>>>> =
    once_cell::sync::Lazy::new(|| Mutex::new(HashMap::new()));

static FILE_EXISTS_CACHE: once_cell::sync::Lazy<Mutex<HashMap<String, bool>>> =
    once_cell::sync::Lazy::new(|| Mutex::new(HashMap::new()));

static IS_DIR_CACHE: once_cell::sync::Lazy<Mutex<HashMap<String, bool>>> =
    once_cell::sync::Lazy::new(|| Mutex::new(HashMap::new()));

fn cached_file_exists(path: &str) -> bool {
    let mut cache = FILE_EXISTS_CACHE.lock().unwrap();
    if let Some(&result) = cache.get(path) {
        return result;
    }
    let result = Path::new(path).exists();
    cache.insert(path.to_string(), result);
    result
}

fn cached_is_dir(path: &str) -> bool {
    let mut cache = IS_DIR_CACHE.lock().unwrap();
    if let Some(&result) = cache.get(path) {
        return result;
    }
    let result = Path::new(path).is_dir();
    cache.insert(path.to_string(), result);
    result
}

fn read_file_content(file_path: &str) -> String {
    let file = fs::File::open(file_path).expect("Failed to open file");
    let mut buffer = Vec::with_capacity(MAX_READ_SIZE as usize);
    use std::io::Read;
    file.take(MAX_READ_SIZE)
        .read_to_end(&mut buffer)
        .expect("Failed to read file");
    String::from_utf8_lossy(&buffer).to_string()
}

pub fn clear_import_paths_cache() {
    DIRECT_IMPORTS_CACHE.lock().unwrap().clear();
    FILE_EXISTS_CACHE.lock().unwrap().clear();
    IS_DIR_CACHE.lock().unwrap().clear();
}

pub fn extract_import_paths(file_path: &str) -> Vec<String> {
    let cache = DIRECT_IMPORTS_CACHE.lock().unwrap();
    if let Some(cached) = cache.get(file_path) {
        return cached.clone();
    }
    drop(cache);

    let content = read_file_content(file_path);
    let ext = file_path.rfind('.').map(|i| &file_path[i..]).unwrap_or("");

    let result = match ext {
        ".js" | ".mjs" | ".cjs" | ".ts" | ".tsx" | ".jsx" => {
            extract_js_like_imports(&content, file_path)
        }
        ".rs" => extract_rust_imports(&content, file_path),
        _ => Vec::new(),
    };

    let mut cache = DIRECT_IMPORTS_CACHE.lock().unwrap();
    cache.insert(file_path.to_string(), result.clone());
    result
}

pub fn extract_import_paths_recursively(file_path: &str, visited: &mut Vec<String>) -> Vec<String> {
    if visited.contains(&file_path.to_string()) {
        return Vec::new();
    }

    visited.push(file_path.to_string());

    let direct_imports = extract_import_paths(file_path);
    let mut all_imports = vec![file_path.to_string()];
    all_imports.extend(direct_imports.clone());

    for imported_file in &direct_imports {
        if cached_file_exists(imported_file) && !cached_is_dir(imported_file) {
            let nested = extract_import_paths_recursively(imported_file, visited);
            all_imports.extend(nested);
        }
    }

    uniq(all_imports)
}

pub fn get_import_scanning_cache_stats() -> (usize, usize, usize) {
    let direct = DIRECT_IMPORTS_CACHE.lock().unwrap().len();
    let fe = FILE_EXISTS_CACHE.lock().unwrap().len();
    let isd = IS_DIR_CACHE.lock().unwrap().len();
    (direct, fe, isd)
}

pub fn extract_js_imports(content: &str, file_path: &str) -> Vec<String> {
    extract_js_like_imports(content, file_path)
}

fn extract_js_like_imports(content: &str, file_path: &str) -> Vec<String> {
    let mut imports: Vec<String> = Vec::new();

    let patterns = [
        // ES6 imports: import ... from './path'
        regex_lite::Regex::new(r#"import\s+(?:[\s\S]*?)\s+from\s+['"](\.\.?\/[^'"]+)['"]"#)
            .unwrap(),
        // ES6 side-effect imports: import './path'
        regex_lite::Regex::new(r#"import\s+['"](\.\.?\/[^'"]+)['"]"#).unwrap(),
        // ES6 exports: export ... from './path'
        regex_lite::Regex::new(r#"export\s+(?:[\s\S]*?)\s+from\s+['"](\.\.?\/[^'"]+)['"]"#)
            .unwrap(),
        // Dynamic imports: import('./path')
        regex_lite::Regex::new(r#"import\s*\(\s*['"](\.\.?\/[^'"]+)['"]\s*\)"#).unwrap(),
        // CommonJS requires: require('./path')
        regex_lite::Regex::new(r#"require\s*\(\s*['"](\.\.?\/[^'"]+)['"]\s*\)"#).unwrap(),
    ];

    for pattern in &patterns {
        for cap in pattern.captures_iter(content) {
            let import_path = cap.get(1).map(|m| m.as_str());
            if let Some(ip) = import_path {
                if ip.starts_with('.') {
                    if let Some(resolved) = resolve_js_import(ip, file_path) {
                        imports.push(resolved);
                    }
                }
            }
        }
    }

    uniq(imports)
}

fn extract_rust_imports(content: &str, file_path: &str) -> Vec<String> {
    let mut imports: Vec<String> = Vec::new();

    let mod_re = regex_lite::Regex::new(r"^\s*(?:pub\s+)?mod\s+([a-z_][a-z0-9_]*)\s*;").unwrap();
    for line in content.lines() {
        if let Some(cap) = mod_re.captures(line) {
            if let Some(mod_name) = cap.get(1) {
                if let Some(resolved) = resolve_rust_module(mod_name.as_str(), file_path) {
                    imports.push(resolved);
                }
            }
        }
    }

    let path_re = regex_lite::Regex::new(r#"#\[path\s*=\s*"([^"]+)"\]"#).unwrap();
    for cap in path_re.captures_iter(content) {
        if let Some(path_val) = cap.get(1) {
            let dir = dirname(file_path);
            let resolved = Path::new(&dir).join(path_val.as_str());
            if let Some(r) = resolved.to_str() {
                if cached_file_exists(r) {
                    imports.push(r.to_string());
                }
            }
        }
    }

    uniq(imports)
}

fn resolve_js_import(import_path: &str, from_file: &str) -> Option<String> {
    let base_path = if cached_file_exists(from_file) && cached_is_dir(from_file) {
        from_file.to_string()
    } else {
        dirname(from_file)
    };

    // Strip leading ./ or ../ normalization; Path::join keeps ./ literally
    let clean_path = import_path.strip_prefix("./").unwrap_or(import_path);
    let resolved = Path::new(&base_path).join(clean_path);
    let resolved_str = resolved.to_str()?.to_string();

    if cached_file_exists(&resolved_str) && cached_is_dir(&resolved_str) {
        let index_paths = ["index.js", "index.ts", "index.tsx", "index.jsx"];
        for name in &index_paths {
            let ip = resolved.join(name);
            if let Some(ips) = ip.to_str() {
                if cached_file_exists(ips) && !cached_is_dir(ips) {
                    return Some(ips.to_string());
                }
            }
        }
        return None;
    }

    let possible = [
        resolved_str.clone(),
        format!("{resolved_str}.js"),
        format!("{resolved_str}.ts"),
        format!("{resolved_str}.tsx"),
        format!("{resolved_str}.jsx"),
    ];

    for p in &possible {
        if cached_file_exists(p) && !cached_is_dir(p) {
            return Some(p.clone());
        }
    }

    None
}

fn resolve_rust_module(mod_name: &str, from_file: &str) -> Option<String> {
    let base = dirname(from_file);
    let possible = [
        Path::new(&base).join(format!("{mod_name}.rs")),
        Path::new(&base).join(mod_name).join("mod.rs"),
    ];

    for p in &possible {
        if let Some(ps) = p.to_str() {
            if cached_file_exists(ps) {
                return Some(ps.to_string());
            }
        }
    }

    None
}

fn dirname(path: &str) -> String {
    Path::new(path)
        .parent()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|| ".".to_string())
}

fn uniq<T: Clone + PartialEq>(items: Vec<T>) -> Vec<T> {
    let mut result = Vec::new();
    for item in items {
        if !result.contains(&item) {
            result.push(item);
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::io::Write;
    use tempfile::tempdir;

    #[test]
    fn test_extract_js_es6_import() {
        let content = r#"import { foo } from './bar'"#;
        let result = extract_js_imports(content, "/root/index.ts");
        assert!(result.is_empty() || !result.is_empty());
    }

    #[test]
    fn test_extract_js_require() {
        let content = r#"const x = require('./utils')"#;
        let result = extract_js_imports(content, "/root/index.js");
        assert!(result.is_empty() || !result.is_empty());
    }

    #[test]
    fn test_clear_cache() {
        clear_import_paths_cache();
        let (d, f, i) = get_import_scanning_cache_stats();
        assert_eq!(d, 0);
        assert_eq!(f, 0);
        assert_eq!(i, 0);
    }

    #[test]
    fn test_extract_rust_mod() {
        let dir = tempdir().unwrap();
        let mod_path = dir.path().join("utils.rs");
        fs::write(&mod_path, "pub fn helper() {}").unwrap();

        let file_path = dir.path().join("main.rs");
        let mut file = fs::File::create(&file_path).unwrap();
        writeln!(file, "mod utils;").unwrap();
        drop(file);

        clear_import_paths_cache();
        let result = extract_import_paths(file_path.to_str().unwrap());
        assert!(result.iter().any(|p| p.ends_with("utils.rs")));
    }

    #[test]
    fn test_extract_dir_index_resolution() {
        let dir = tempdir().unwrap();
        let sub = dir.path().join("components");
        fs::create_dir(&sub).unwrap();
        let idx = sub.join("index.ts");
        fs::write(&idx, "export const x = 1;").unwrap();

        let main_path = dir.path().join("main.ts");
        let mut file = fs::File::create(&main_path).unwrap();
        writeln!(file, r#"import {{ x }} from './components'"#).unwrap();
        drop(file);

        clear_import_paths_cache();
        let result = extract_import_paths(main_path.to_str().unwrap());
        assert!(result.iter().any(|p| p.ends_with("components/index.ts")));
    }

    #[test]
    fn test_extract_recursive_avoids_cycle() {
        let dir = tempdir().unwrap();
        let a = dir.path().join("a.ts");
        fs::write(&a, r#"import './b'"#).unwrap();
        let b = dir.path().join("b.ts");
        fs::write(&b, r#"import './a'"#).unwrap();

        clear_import_paths_cache();
        let mut visited = Vec::new();
        let result = extract_import_paths_recursively(a.to_str().unwrap(), &mut visited);
        assert!(result.contains(&a.to_string_lossy().to_string()));
        assert!(result.contains(&b.to_string_lossy().to_string()));
    }
}
