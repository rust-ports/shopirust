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
        AiInstructions::All => vec![
            (".github/copilot-instructions.md", "../../AGENTS.md"),
            (".cursor/rules/AGENTS.md", "../../AGENTS.md"),
            ("CLAUDE.md", "AGENTS.md"),
        ],
        AiInstructions::VsCode => vec![(".github/copilot-instructions.md", "../../AGENTS.md")],
        AiInstructions::Cursor => vec![(".cursor/rules/AGENTS.md", "../../AGENTS.md")],
        AiInstructions::Claude => vec![("CLAUDE.md", "AGENTS.md")],
        AiInstructions::Skip => vec![],
    }
}
