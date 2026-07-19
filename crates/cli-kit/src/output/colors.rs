use colored::Colorize;

/// Apply cyan color.
pub fn cyan(text: &str) -> String {
    text.cyan().to_string()
}

/// Apply gray (dimmed) color.
pub fn gray(text: &str) -> String {
    text.dimmed().to_string()
}

/// Apply magenta color.
pub fn magenta(text: &str) -> String {
    text.magenta().to_string()
}

/// Apply bright magenta color.
pub fn magenta_bright(text: &str) -> String {
    text.bright_magenta().to_string()
}

/// Apply green color.
pub fn green(text: &str) -> String {
    text.green().to_string()
}

/// Apply yellow color.
pub fn yellow(text: &str) -> String {
    text.yellow().to_string()
}

/// Apply red color.
pub fn red(text: &str) -> String {
    text.red().to_string()
}

/// Apply bright red color.
pub fn red_bright(text: &str) -> String {
    text.bright_red().to_string()
}

/// Apply blue color.
pub fn blue(text: &str) -> String {
    text.blue().to_string()
}

/// Apply bright blue color.
pub fn blue_bright(text: &str) -> String {
    text.bright_blue().to_string()
}

/// Apply bold modifier.
pub fn bold(text: &str) -> String {
    text.bold().to_string()
}

/// Apply italic modifier.
pub fn italic(text: &str) -> String {
    text.italic().to_string()
}

/// Apply underline modifier.
pub fn underline(text: &str) -> String {
    text.underline().to_string()
}

/// Apply dimmed modifier.
pub fn dim(text: &str) -> String {
    text.dimmed().to_string()
}

/// Produce an ANSI-reset sequence.
pub fn reset() -> String {
    "\x1b[0m".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cyan_contains_text() {
        let out = cyan("hello");
        assert!(out.contains("hello"));
    }

    #[test]
    fn test_gray_contains_text() {
        let out = gray("world");
        assert!(out.contains("world"));
    }

    #[test]
    fn test_green_contains_text() {
        let out = green("ok");
        assert!(out.contains("ok"));
    }

    #[test]
    fn test_magenta_bright_applies_ansi() {
        colored::control::set_override(true);
        let out = magenta_bright("test");
        assert!(out.starts_with("\x1b["));
    }

    #[test]
    fn test_bold_output() {
        let out = bold("bold");
        assert!(out.contains("bold"));
    }

    #[test]
    fn test_reset_eq_ansi_reset() {
        assert_eq!(reset(), "\x1b[0m");
    }

    #[test]
    fn test_dim_contains_text() {
        let out = dim("faded");
        assert!(out.contains("faded"));
    }

    #[test]
    fn test_italic_contains_text() {
        let out = italic("emphasis");
        assert!(out.contains("emphasis"));
    }
}
