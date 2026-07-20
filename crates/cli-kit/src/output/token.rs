use colored::Colorize;

#[derive(Debug, Clone)]
pub enum Token {
    Raw(String),
    Command(String),
    Link {
        label: Option<String>,
        url: String,
    },
    UserInput(String),
    Subdued(String),
    FilePath(String),
    Bold(String),
    Info(String),
    Warn(String),
    Error(String),
    Char(char),
    List {
        title: Option<String>,
        items: Vec<Vec<Token>>,
        ordered: bool,
    },
}

fn render_token_styled(t: &Token) -> String {
    match t {
        Token::Raw(s) => s.clone(),
        Token::Command(s) => format!("`{}`", s).magenta().to_string(),
        Token::Link { label, url } => {
            let text = label.clone().unwrap_or_else(|| url.clone());
            format!("{} ({})", text, url)
        }
        Token::UserInput(s) => s.cyan().to_string(),
        Token::Subdued(s) => s.dimmed().to_string(),
        Token::FilePath(s) => s.italic().to_string(),
        Token::Bold(s) => s.bold().to_string(),
        Token::Info(s) => s.blue().to_string(),
        Token::Warn(s) => s.yellow().to_string(),
        Token::Error(s) => s.red().to_string(),
        Token::Char(c) => c.to_string(),
        Token::List {
            title,
            items,
            ordered,
        } => {
            let mut out = String::new();
            if let Some(t) = title {
                out.push_str(t);
                out.push('\n');
            }
            for (i, item) in items.iter().enumerate() {
                let bullet = if *ordered {
                    format!("{}. ", i + 1)
                } else {
                    "• ".to_string()
                };
                out.push_str(&bullet);
                out.push_str(&render_tokens_styled(item));
                out.push('\n');
            }
            out
        }
    }
}

fn render_token_plain(t: &Token) -> String {
    match t {
        Token::Raw(s) => s.clone(),
        Token::Command(s) => format!("`{}`", s),
        Token::Link { label, url } => label.clone().unwrap_or(url.clone()),
        Token::UserInput(s) => s.clone(),
        Token::Subdued(s) => s.clone(),
        Token::FilePath(s) => s.clone(),
        Token::Bold(s) => s.clone(),
        Token::Info(s) => s.clone(),
        Token::Warn(s) => s.clone(),
        Token::Error(s) => s.clone(),
        Token::Char(c) => c.to_string(),
        Token::List {
            title,
            items,
            ordered,
        } => {
            let mut out = String::new();
            if let Some(t) = title {
                out.push_str(t);
                out.push('\n');
            }
            for (i, item) in items.iter().enumerate() {
                let bullet = if *ordered {
                    format!("{}. ", i + 1)
                } else {
                    "- ".to_string()
                };
                out.push_str(&bullet);
                out.push_str(&render_tokens_plain(item));
                out.push('\n');
            }
            out
        }
    }
}

pub fn render_tokens_styled(tokens: &[Token]) -> String {
    let mut out = String::new();
    for t in tokens {
        out.push_str(&render_token_styled(t));
    }
    out
}

pub fn render_tokens_plain(tokens: &[Token]) -> String {
    let mut out = String::new();
    for t in tokens {
        out.push_str(&render_token_plain(t));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_render_plain_raw() {
        assert_eq!(render_tokens_plain(&[Token::Raw("hello".into())]), "hello");
    }

    #[test]
    fn test_render_plain_command() {
        assert_eq!(
            render_tokens_plain(&[Token::Command("dev".into())]),
            "`dev`"
        );
    }

    #[test]
    fn test_render_plain_link() {
        let t = Token::Link {
            label: Some("Shopify".into()),
            url: "https://shopify.com".into(),
        };
        assert_eq!(render_tokens_plain(&[t]), "Shopify");
    }

    #[test]
    fn test_render_styled_has_ansi() {
        colored::control::set_override(true);
        let out = render_tokens_styled(&[Token::Command("dev".into())]);
        assert!(out.starts_with("\x1b["));
        assert!(out.contains("`dev`"));
        assert!(out.ends_with("\x1b[0m"));
    }

    #[test]
    fn test_render_tokens_multiple() {
        let tokens = vec![
            Token::Bold("Hello".into()),
            Token::Raw(" ".into()),
            Token::Warn("world".into()),
        ];
        let plain = render_tokens_plain(&tokens);
        assert_eq!(plain, "Hello world");
    }

    #[test]
    fn test_render_plain_list_ordered() {
        let tokens = [Token::List {
            title: Some("Numbers".into()),
            items: vec![
                vec![Token::Raw("one".into())],
                vec![Token::Raw("two".into())],
            ],
            ordered: true,
        }];
        let out = render_tokens_plain(&tokens);
        assert_eq!(out, "Numbers\n1. one\n2. two\n");
    }

    #[test]
    fn test_render_plain_list_unordered() {
        let tokens = [Token::List {
            title: None,
            items: vec![
                vec![Token::Raw("foo".into())],
                vec![Token::Raw("bar".into())],
            ],
            ordered: false,
        }];
        let out = render_tokens_plain(&tokens);
        assert_eq!(out, "- foo\n- bar\n");
    }
}
