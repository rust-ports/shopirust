use crate::output::tokens::TokenItem;

/// Render a file path (italic).
/// If possible, relativizes the path against the current working directory.
pub fn render_file_path(path: &str) -> TokenItem {
    let display = relativize_path(path);
    TokenItem::file_path(display)
}

fn relativize_path(path: &str) -> String {
    if let Ok(cwd) = std::env::current_dir() {
        let p = std::path::Path::new(path);
        if let Ok(relative) = p.strip_prefix(&cwd) {
            return relative.display().to_string();
        }
    }
    path.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_render_file_path_plain() {
        let t = render_file_path("/some/path");
        assert!(t.render_plain().contains("path"));
    }

    #[test]
    fn test_render_file_path_ansi() {
        colored::control::set_override(true);
        let t = render_file_path("/tmp/file.rs");
        let out = t.render_ansi(true);
        assert!(out.starts_with("\x1b["));
    }
}
