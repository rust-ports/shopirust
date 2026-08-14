//! URL / name validators used by app config schemas.

pub const APP_NAME_MAX_LENGTH: usize = 30;

pub fn is_valid_name(name: &str) -> bool {
    name.chars().count() <= APP_NAME_MAX_LENGTH
}

pub fn is_valid_url(input: &str, https_only: bool) -> bool {
    if input.contains('\n') {
        return false;
    }
    let Ok(url) = url::Url::parse(input) else {
        return false;
    };
    match url.scheme() {
        "https" => true,
        "http" => !https_only,
        _ => false,
    }
}

pub fn validate_relative_url(input: &str) -> bool {
    input.starts_with('/') || is_valid_url(input, true)
}

pub fn ensure_path_starts_with_slash(arg: &str) -> String {
    if arg.starts_with('/') {
        arg.to_string()
    } else {
        format!("/{arg}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn urls() {
        assert!(is_valid_url("https://example.com", false));
        assert!(is_valid_url("http://example.com", false));
        assert!(!is_valid_url("http://example.com", true));
        assert!(!is_valid_url("https://example.com\n", false));
        assert!(!is_valid_url("ftp://example.com", false));
        assert!(!is_valid_url("example.com", false));
    }

    #[test]
    fn relative() {
        assert!(validate_relative_url("/callback"));
        assert!(validate_relative_url("https://example.com/cb"));
        assert!(!validate_relative_url("http://example.com/cb"));
    }

    #[test]
    fn slash_prefix() {
        assert_eq!(ensure_path_starts_with_slash("apps"), "/apps");
        assert_eq!(ensure_path_starts_with_slash("/apps"), "/apps");
    }

    #[test]
    fn name_length() {
        assert!(is_valid_name("hello"));
        assert!(!is_valid_name(&"a".repeat(31)));
    }
}
