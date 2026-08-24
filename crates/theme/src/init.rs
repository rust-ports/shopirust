use std::path::{Path, PathBuf};

pub const DEFAULT_CLONE_URL: &str = "https://github.com/Shopify/skeleton-theme.git";
pub const INSTRUCTIONS_URL: &str = "https://github.com/Shopify/theme-liquid-docs.git";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AiInstructions {
    All,
    VsCode,
    Cursor,
    Claude,
    Skip,
}

pub fn destination(base: impl AsRef<Path>, name: &str) -> PathBuf {
    base.as_ref().join(name)
}

pub fn is_populated(path: impl AsRef<Path>) -> bool {
    path.as_ref()
        .read_dir()
        .ok()
        .and_then(|mut entries| entries.next())
        .is_some()
}

pub fn skeleton_cleanup_paths() -> &'static [&'static str] {
    &[".git", ".github", ".cursor", ".claude", ".all"]
}

pub fn instruction_links(choice: AiInstructions) -> Vec<(&'static str, &'static str)> {
    match choice {
        // Upstream maps github → `copilot-instructions.md` at the theme root.
        AiInstructions::All => vec![
            ("copilot-instructions.md", "AGENTS.md"),
            ("CLAUDE.md", "AGENTS.md"),
        ],
        AiInstructions::VsCode => vec![("copilot-instructions.md", "AGENTS.md")],
        AiInstructions::Cursor => vec![],
        AiInstructions::Claude => vec![("CLAUDE.md", "AGENTS.md")],
        AiInstructions::Skip => vec![],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn destination_joins_base_and_name() {
        let base = PathBuf::from("/tmp/theme");
        assert_eq!(
            destination(base, "my-theme"),
            PathBuf::from("/tmp/theme/my-theme")
        );
    }

    #[test]
    fn is_populated_returns_false_for_missing_directory() {
        assert!(!is_populated("/nonexistent/path/12345"));
    }

    #[test]
    fn is_populated_returns_false_for_empty_directory() {
        let temp = tempfile::tempdir().unwrap();
        assert!(!is_populated(temp.path()));
    }

    #[test]
    fn is_populated_returns_true_for_non_empty_directory() {
        let temp = tempfile::tempdir().unwrap();
        fs::write(temp.path().join("file.txt"), "content").unwrap();
        assert!(is_populated(temp.path()));
    }

    #[test]
    fn skeleton_cleanup_paths_returns_expected_paths() {
        let paths = skeleton_cleanup_paths();
        assert_eq!(paths, &[".git", ".github", ".cursor", ".claude", ".all"]);
    }

    #[test]
    fn instruction_links_all_creates_all_instruction_files() {
        let links = instruction_links(AiInstructions::All);
        assert_eq!(links.len(), 2);
        assert!(links.contains(&("copilot-instructions.md", "AGENTS.md")));
        assert!(links.contains(&("CLAUDE.md", "AGENTS.md")));
    }

    #[test]
    fn instruction_links_vscode_creates_only_vscode_instruction() {
        let links = instruction_links(AiInstructions::VsCode);
        assert_eq!(links, vec![("copilot-instructions.md", "AGENTS.md")]);
    }

    #[test]
    fn instruction_links_cursor_creates_only_cursor_instruction() {
        let links = instruction_links(AiInstructions::Cursor);
        assert!(links.is_empty());
    }

    #[test]
    fn instruction_links_claude_creates_only_claude_instruction() {
        let links = instruction_links(AiInstructions::Claude);
        assert_eq!(links, vec![("CLAUDE.md", "AGENTS.md")]);
    }

    #[test]
    fn instruction_links_skip_returns_empty() {
        assert!(instruction_links(AiInstructions::Skip).is_empty());
    }
}
