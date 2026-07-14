use rand::seq::SliceRandom;
use chrono::{DateTime, Local, Utc};

const ADJECTIVES: &[&str] = &[
    "swift", "bright", "calm", "eager", "fierce", "gentle", "happy", "keen",
    "lively", "mighty", "noble", "proud", "quick", "sharp", "tough", "warm",
];

const NOUNS: &[&str] = &[
    "falcon", "beacon", "crystal", "dragon", "ember", "forest", "garden",
    "harbor", "island", "jungle", "knight", "lunar", "marble", "nebula",
    "ocean", "phoenix", "quartz", "raven", "storm", "temple",
];

pub fn get_random_name() -> String {
    let adj = ADJECTIVES.choose(&mut rand::thread_rng()).unwrap_or(&"swift");
    let noun = NOUNS.choose(&mut rand::thread_rng()).unwrap_or(&"falcon");
    format!("{}-{}", adj, noun)
}

pub fn capitalize(str: &str) -> String {
    let mut chars = str.chars();
    match chars.next() {
        None => String::new(),
        Some(first) => first.to_uppercase().to_string() + chars.as_str(),
    }
}

pub fn try_parse_int(maybe_int: &str) -> Option<i32> {
    maybe_int.trim().parse::<i32>().ok()
}

pub fn slugify(str: &str) -> String {
    str.to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() || c == '-' { c } else { '-' })
        .collect::<String>()
        .trim_matches('-')
        .to_string()
}

pub fn escape_regex(str: &str) -> String {
    let mut out = String::with_capacity(str.len());
    for c in str.chars() {
        match c {
            '\\' | '.' | '+' | '*' | '?' | '(' | ')' | '|' | '[' | ']'
            | '{' | '}' | '^' | '$' | '#' | '&' | '-' | '~' => {
                out.push('\\');
                out.push(c);
            }
            _ => out.push(c),
        }
    }
    out
}

pub fn camelize(input: &str) -> String {
    let mut out = String::new();
    let mut upper = false;
    for c in input.chars() {
        if c == '_' || c == '-' || c == ' ' {
            upper = true;
        } else if upper {
            out.push(c.to_ascii_uppercase());
            upper = false;
        } else {
            out.push(c);
        }
    }
    out
}

pub fn capitalize_words(input: &str) -> String {
    input
        .split(|c: char| c == '_' || c == '-' || c == ' ')
        .filter(|w| !w.is_empty())
        .map(capitalize)
        .collect::<Vec<_>>()
        .join(" ")
}

pub fn hyphenate(input: &str) -> String {
    input
        .chars()
        .map(|c| {
            if c.is_uppercase() {
                format!("-{}", c.to_ascii_lowercase())
            } else if c == '_' || c == ' ' {
                "-".to_string()
            } else {
                c.to_string()
            }
        })
        .collect::<String>()
        .trim_matches('-')
        .to_string()
}

pub fn underscore(input: &str) -> String {
    input
        .chars()
        .map(|c| {
            if c.is_uppercase() {
                format!("_{}", c.to_ascii_lowercase())
            } else if c == '-' || c == ' ' {
                "_".to_string()
            } else {
                c.to_string()
            }
        })
        .collect::<String>()
        .trim_matches('_')
        .to_string()
}

pub fn constantize(input: &str) -> String {
    underscore(input).to_uppercase()
}

pub fn pascalize(str: &str) -> String {
    let camel = camelize(str);
    let mut chars = camel.chars();
    match chars.next() {
        None => String::new(),
        Some(first) => first.to_uppercase().to_string() + chars.as_str(),
    }
}

pub fn format_date(dt: &DateTime<Utc>) -> String {
    dt.format("%Y-%m-%d %H:%M:%S").to_string()
}

pub fn format_local_date(date_string: &str) -> String {
    if let Ok(utc) = DateTime::parse_from_str(&format!("{} +0000", date_string), "%Y-%m-%d %H:%M:%S %z") {
        let local: DateTime<Local> = utc.with_timezone(&Local);
        local.format("%Y-%m-%d %H:%M:%S").to_string()
    } else {
        date_string.to_string()
    }
}

pub fn normalize_delimited_string(delimited_string: Option<&str>, delimiter: char) -> String {
    match delimited_string {
        None => String::new(),
        Some(s) => {
            let mut parts: Vec<&str> = s.split(delimiter).map(|p| p.trim()).filter(|p| !p.is_empty()).collect();
            parts.sort();
            parts.dedup();
            parts.join(&delimiter.to_string())
        }
    }
}

pub fn time_ago(from: &DateTime<Utc>, to: &DateTime<Utc>) -> String {
    let secs = (*to - *from).num_seconds().abs();
    if secs < 60 {
        format!("{} seconds ago", secs)
    } else if secs < 3600 {
        format!("{} minutes ago", secs / 60)
    } else if secs < 86400 {
        format!("{} hours ago", secs / 3600)
    } else {
        format!("{} days ago", secs / 86400)
    }
}

pub fn lines_to_columns(lines: &[Vec<String>]) -> String {
    if lines.is_empty() {
        return String::new();
    }
    let col_count = lines.iter().map(|row| row.len()).max().unwrap_or(0);
    let widths: Vec<usize> = (0..col_count)
        .map(|col| lines.iter().filter_map(|row| row.get(col)).map(|s| s.len()).max().unwrap_or(0))
        .collect();

    let mut out = String::new();
    for row in lines {
        for (i, cell) in row.iter().enumerate() {
            out.push_str(cell);
            if i < col_count - 1 {
                let pad = widths[i].saturating_sub(cell.len()) + 2;
                for _ in 0..pad {
                    out.push(' ');
                }
            }
        }
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_capitalize() {
        assert_eq!(capitalize("hello"), "Hello");
        assert_eq!(capitalize(""), "");
    }

    #[test]
    fn test_try_parse_int_valid() {
        assert_eq!(try_parse_int("42"), Some(42));
    }

    #[test]
    fn test_try_parse_int_invalid() {
        assert_eq!(try_parse_int("abc"), None);
    }

    #[test]
    fn test_slugify() {
        assert_eq!(slugify("Hello World"), "hello-world");
    }

    #[test]
    fn test_camelize() {
        assert_eq!(camelize("hello_world"), "helloWorld");
        assert_eq!(camelize("hello-world"), "helloWorld");
    }

    #[test]
    fn test_underscore() {
        assert_eq!(underscore("helloWorld"), "hello_world");
        assert_eq!(underscore("HelloWorld"), "hello_world");
    }

    #[test]
    fn test_hyphenate() {
        assert_eq!(hyphenate("helloWorld"), "hello-world");
    }

    #[test]
    fn test_constantize() {
        assert_eq!(constantize("helloWorld"), "HELLO_WORLD");
    }

    #[test]
    fn test_pascalize() {
        assert_eq!(pascalize("hello_world"), "HelloWorld");
    }

    #[test]
    fn test_capitalize_words() {
        assert_eq!(capitalize_words("hello_world"), "Hello World");
    }

    #[test]
    fn test_get_random_name() {
        let name = get_random_name();
        assert!(name.contains('-'));
    }

    #[test]
    fn test_normalize_delimited_string() {
        let result = normalize_delimited_string(Some(" b, a, c, b "), ',');
        assert_eq!(result, "a,b,c");
    }

    #[test]
    fn test_time_ago_seconds() {
        let now = Utc::now();
        let past = now - chrono::Duration::seconds(30);
        assert!(time_ago(&past, &now).contains("30"));
    }

    #[test]
    fn test_lines_to_columns() {
        let lines = vec![
            vec!["a".into(), "bbb".into()],
            vec!["cc".into(), "d".into()],
        ];
        let result = lines_to_columns(&lines);
        assert!(result.contains("a"));
        assert!(result.contains("bbb"));
    }
}
