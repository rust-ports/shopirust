use crate::error::AppError;

const MAX_LEN: usize = 100;

/// Validate an optional version tag (`app deploy --version` / `app release`).
pub fn validate_version(version: Option<&str>) -> Result<(), AppError> {
    let Some(version) = version else {
        return Ok(());
    };
    if version == "." || version == ".." {
        return Err(invalid(version, "Version name cannot be '.' or '..'."));
    }
    if version.len() > MAX_LEN {
        return Err(invalid(
            version,
            "Version name must be 100 characters or less.",
        ));
    }
    if !version
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_'))
    {
        return Err(invalid(
            version,
            "Version name can only contain letters, numbers, dots, hyphens, and underscores.",
        ));
    }
    Ok(())
}

fn invalid(input: &str, hint: &str) -> AppError {
    AppError::message(format!("Invalid version name: {input}\n{hint}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_allowed_charset() {
        validate_version(Some("AZaz09.-_")).unwrap();
        validate_version(None).unwrap();
    }

    #[test]
    fn rejects_dot_and_dotdot() {
        assert!(validate_version(Some(".")).is_err());
        assert!(validate_version(Some("..")).is_err());
    }

    #[test]
    fn rejects_special_chars() {
        assert!(validate_version(Some("AZa%&\n/")).is_err());
    }

    #[test]
    fn rejects_too_long() {
        let s = "A".repeat(101);
        assert!(validate_version(Some(&s)).is_err());
    }
}
