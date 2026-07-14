use crate::util::system;

pub struct GitHubRemote {
    pub full_name: String,
    pub owner: String,
    pub repo: String,
}

pub async fn detect_current_branch(path: &std::path::Path) -> Option<String> {
    let branch = system::capture_output(
        "git",
        &["-C", path.to_str().unwrap_or("."), "rev-parse", "--abbrev-ref", "HEAD"],
    )
    .await
    .ok()?;
    if branch.is_empty() || branch == "HEAD" {
        return None;
    }
    Some(branch)
}

pub async fn detect_github_remote(path: &std::path::Path) -> Option<GitHubRemote> {
    let remote = system::capture_output(
        "git",
        &["-C", path.to_str().unwrap_or("."), "config", "--get", "remote.origin.url"],
    )
    .await
    .ok()?;

    if remote.is_empty() {
        return None;
    }

    let full_name = extract_github_full_name(&remote)?;
    let parts: Vec<&str> = full_name.split('/').collect();
    if parts.len() != 2 {
        return None;
    }

    Some(GitHubRemote {
        full_name: full_name.clone(),
        owner: parts[0].to_string(),
        repo: parts[1].to_string(),
    })
}

pub async fn has_git() -> bool {
    system::capture_output("git", &["--version"]).await.is_ok()
}

fn extract_github_full_name(remote_url: &str) -> Option<String> {
    let remote = remote_url.trim();
    let name = if let Some(name) = remote.strip_prefix("git@github.com:") {
        Some(name.to_string())
    } else if let Some(name) = remote.strip_prefix("https://github.com/") {
        Some(name.to_string())
    } else {
        None
    }?;
    Some(name.strip_suffix(".git").unwrap_or(&name).to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_github_full_name_ssh() {
        let result = extract_github_full_name("git@github.com:owner/repo.git");
        assert_eq!(result, Some("owner/repo".into()));
    }

    #[test]
    fn test_extract_github_full_name_https() {
        let result = extract_github_full_name("https://github.com/owner/repo.git");
        assert_eq!(result, Some("owner/repo".into()));
    }

    #[test]
    fn test_extract_github_full_name_no_git_suffix() {
        let result = extract_github_full_name("git@github.com:owner/repo");
        assert_eq!(result, Some("owner/repo".into()));
    }

    #[test]
    fn test_extract_github_full_name_invalid() {
        let result = extract_github_full_name("https://other.com/repo.git");
        assert_eq!(result, None);
    }

    #[test]
    fn test_extract_github_full_name_empty() {
        let result = extract_github_full_name("");
        assert_eq!(result, None);
    }
}
