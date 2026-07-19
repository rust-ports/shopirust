pub struct TextInput {
    value: String,
    placeholder: String,
    password: bool,
    cursor_pos: usize,
}

impl TextInput {
    pub fn new(value: impl Into<String>) -> Self {
        Self {
            value: value.into(),
            placeholder: String::new(),
            password: false,
            cursor_pos: 0,
        }
    }

    pub fn with_placeholder(mut self, placeholder: impl Into<String>) -> Self {
        self.placeholder = placeholder.into();
        self
    }

    pub fn with_password(mut self, password: bool) -> Self {
        self.password = password;
        self
    }

    pub fn value(&self) -> &str {
        &self.value
    }

    pub fn cursor_pos(&self) -> usize {
        self.cursor_pos.min(self.value.len())
    }

    pub fn insert(&mut self, ch: char) {
        let pos = self.cursor_pos().min(self.value.len());
        self.value.insert(pos, ch);
        self.cursor_pos = pos + 1;
    }

    pub fn insert_str(&mut self, s: &str) {
        let pos = self.cursor_pos().min(self.value.len());
        self.value.insert_str(pos, s);
        self.cursor_pos = pos + s.len();
    }

    pub fn delete_before(&mut self) {
        let pos = self.cursor_pos();
        if pos > 0 {
            self.value.remove(pos - 1);
            self.cursor_pos = pos - 1;
        }
    }

    pub fn delete_after(&mut self) {
        let pos = self.cursor_pos();
        if pos < self.value.len() {
            self.value.remove(pos);
        }
    }

    pub fn cursor_left(&mut self) {
        if self.cursor_pos > 0 {
            self.cursor_pos -= 1;
        }
    }

    pub fn cursor_right(&mut self) {
        if self.cursor_pos < self.value.len() {
            self.cursor_pos += 1;
        }
    }

    pub fn cursor_home(&mut self) {
        self.cursor_pos = 0;
    }

    pub fn cursor_end(&mut self) {
        self.cursor_pos = self.value.len();
    }

    pub fn clear(&mut self) {
        self.value.clear();
        self.cursor_pos = 0;
    }

    pub fn rendered_value(&self) -> String {
        let display = if self.password {
            "*".repeat(self.value.len())
        } else {
            self.value.clone()
        };

        let pos = self.cursor_pos().min(display.len());

        let mut out = String::new();
        for (i, ch) in display.chars().enumerate() {
            if i == pos {
                out.push_str(&format!("\x1b[7m{ch}\x1b[0m"));
            } else {
                out.push(ch);
            }
        }
        if pos == display.len() {
            out.push_str("\x1b[7m \x1b[0m");
        }
        out
    }

    pub fn render_placeholder(&self) -> String {
        if self.placeholder.is_empty() {
            return "\x1b[7m \x1b[0m".to_string();
        }
        let placeholder = if self.password {
            "*".repeat(self.placeholder.len())
        } else {
            self.placeholder.clone()
        };
        let first = placeholder.chars().next().map(|c| format!("\x1b[7m{c}\x1b[0m")).unwrap_or_default();
        let rest = format!("\x1b[2m{}\x1b[0m", &placeholder.chars().skip(1).collect::<String>());
        format!("{first}{rest}")
    }
}

pub fn prompt_text_input(prompt: &str, placeholder: Option<&str>, password: bool) -> Result<String, String> {
    if password {
        let inquirer = inquire::Password::new(prompt);
        inquirer.prompt().map_err(|e| e.to_string())
    } else {
        let mut inquirer = inquire::Text::new(prompt);
        if let Some(p) = placeholder {
            inquirer = inquirer.with_placeholder(p);
        }
        inquirer.prompt().map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_text_input_new() {
        let input = TextInput::new("hello");
        assert_eq!(input.value(), "hello");
        assert_eq!(input.cursor_pos(), 0);
    }

    #[test]
    fn test_text_input_insert() {
        let mut input = TextInput::new("helo");
        input.cursor_pos = 3;
        input.insert('l');
        assert_eq!(input.value(), "hello");
        assert_eq!(input.cursor_pos(), 4);
    }

    #[test]
    fn test_text_input_delete_before() {
        let mut input = TextInput::new("hello");
        input.cursor_pos = 5;
        input.delete_before();
        assert_eq!(input.value(), "hell");
        assert_eq!(input.cursor_pos(), 4);
    }

    #[test]
    fn test_text_input_delete_after() {
        let mut input = TextInput::new("hello");
        input.cursor_pos = 2;
        input.delete_after();
        assert_eq!(input.value(), "helo");
        assert_eq!(input.cursor_pos(), 2);
    }

    #[test]
    fn test_text_input_cursor_movement() {
        let mut input = TextInput::new("hello");
        input.cursor_right();
        assert_eq!(input.cursor_pos(), 1);
        input.cursor_left();
        assert_eq!(input.cursor_pos(), 0);
        input.cursor_end();
        assert_eq!(input.cursor_pos(), 5);
        input.cursor_home();
        assert_eq!(input.cursor_pos(), 0);
    }

    #[test]
    fn test_text_input_password() {
        let input = TextInput::new("secret").with_password(true);
        let rendered = input.rendered_value();
        assert_eq!(rendered.chars().filter(|&c| c == '*').count(), 6);
    }

    #[test]
    fn test_text_input_clear() {
        let mut input = TextInput::new("hello");
        input.clear();
        assert_eq!(input.value(), "");
        assert_eq!(input.cursor_pos(), 0);
    }

    #[test]
    fn test_placeholder_renders() {
        let input = TextInput::new("").with_placeholder("type here");
        let rendered = input.render_placeholder();
        assert!(!rendered.is_empty());
        assert!(rendered.contains("ype here"));
    }
}
