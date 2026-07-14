use std::time::Duration;
use tokio::io::AsyncReadExt;
use tokio::process::Command;

pub async fn sleep(seconds: f64) {
    let ms = (seconds * 1000.0) as u64;
    tokio::time::sleep(Duration::from_millis(ms)).await;
}

pub struct CaptureOutputResult {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
}

pub async fn capture_output(command: &str, args: &[&str]) -> Result<String, std::io::Error> {
    let output = Command::new(command).args(args).output().await?;
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

pub async fn capture_output_with_exit_code(
    command: &str,
    args: &[&str],
) -> Result<CaptureOutputResult, std::io::Error> {
    let output = Command::new(command).args(args).output().await?;
    Ok(CaptureOutputResult {
        stdout: String::from_utf8_lossy(&output.stdout).trim().to_string(),
        stderr: String::from_utf8_lossy(&output.stderr).trim().to_string(),
        exit_code: output.status.code().unwrap_or(-1),
    })
}

pub async fn capture_command_with_exit_code(
    full_command: &str,
) -> Result<CaptureOutputResult, std::io::Error> {
    #[cfg(unix)]
    {
        capture_output_with_exit_code("sh", &["-c", full_command]).await
    }
    #[cfg(not(unix))]
    {
        capture_output_with_exit_code("cmd", &["/C", full_command]).await
    }
}

pub async fn exec_command(command: &str) -> Result<(), std::io::Error> {
    let result = capture_command_with_exit_code(command).await?;
    if result.exit_code != 0 {
        return Err(std::io::Error::other(result.stderr));
    }
    Ok(())
}

pub async fn exec(command: &str, args: &[&str]) -> Result<(), std::io::Error> {
    let status = Command::new(command).args(args).status().await?;
    if !status.success() {
        return Err(std::io::Error::other(format!(
            "Command exited with code {:?}",
            status.code()
        )));
    }
    Ok(())
}

pub async fn open_url(url: &str) -> bool {
    open::that(url).is_ok()
}

pub fn terminal_supports_hyperlinks() -> bool {
    std::env::var("TERM").map(|t| t != "dumb").unwrap_or(false)
        && is_terminal::is_terminal(std::io::stdout())
}

pub fn terminal_supports_prompting() -> bool {
    !is_ci()
        && is_terminal::is_terminal(std::io::stdin())
        && is_terminal::is_terminal(std::io::stdout())
}

pub fn is_ci() -> bool {
    std::env::var("CI").is_ok()
        || std::env::var("TF_BUILD").is_ok()
        || std::env::var("GITHUB_ACTIONS").is_ok()
        || std::env::var("GITLAB_CI").is_ok()
        || std::env::var("CIRCLECI").is_ok()
}

pub async fn is_wsl() -> bool {
    #[cfg(target_os = "linux")]
    {
        let result = capture_output("uname", &["-r"]).await.unwrap_or_default();
        result.to_lowercase().contains("microsoft") || result.to_lowercase().contains("wsl")
    }
    #[cfg(not(target_os = "linux"))]
    {
        false
    }
}

pub fn is_stdin_piped() -> bool {
    use std::io::IsTerminal;
    !std::io::stdin().is_terminal()
}

pub async fn read_stdin_string() -> Option<String> {
    if !is_stdin_piped() {
        return None;
    }
    let mut buf = String::with_capacity(1024);
    let mut reader = tokio::io::BufReader::new(tokio::io::stdin());
    reader.read_to_string(&mut buf).await.ok()?;
    if buf.is_empty() {
        None
    } else {
        Some(buf)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_capture_output_echo() {
        let result = capture_output("echo", &["hello"]).await.unwrap();
        assert_eq!(result, "hello");
    }

    #[tokio::test]
    async fn test_sleep_doesnt_panic() {
        sleep(0.001).await;
    }

    #[test]
    fn test_is_ci_not_set() {
        let ci_vars = ["CI", "TF_BUILD", "GITHUB_ACTIONS", "GITLAB_CI", "CIRCLECI"];
        let saved: Vec<(&str, Option<String>)> = ci_vars
            .iter()
            .map(|&v| (v, std::env::var(v).ok()))
            .collect();
        for (name, _) in &saved {
            std::env::remove_var(name);
        }
        assert!(!is_ci());
        for (name, val) in saved {
            if let Some(v) = val {
                std::env::set_var(name, v);
            }
        }
    }

    #[test]
    fn test_terminal_supports_hyperlinks_no_panic() {
        let _ = terminal_supports_hyperlinks();
    }
}
