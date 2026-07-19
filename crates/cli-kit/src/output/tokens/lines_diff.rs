use super::token_item::TokenItem;

/// A single change in a line diff (port of upstream's `Change` type).
#[derive(Debug, Clone, PartialEq)]
pub enum Change {
    Addition(String),
    Deletion(String),
    Noop(String),
}

/// Renders an array of `Change` values as green `+` / magenta `-` lines.
/// Maps to upstream's `linesDiffContentToken`.
pub fn render_lines_diff(changes: &[Change], colors_enabled: bool) -> Vec<TokenItem> {
    let mut items = Vec::with_capacity(changes.len());
    for change in changes {
        match change {
            Change::Addition(line) => {
                let text = format!("+ {line}");
                items.push(TokenItem {
                    value: text,
                    style: if colors_enabled {
                        super::token_item::TokenStyle::Green
                    } else {
                        super::token_item::TokenStyle::Raw
                    },
                });
            }
            Change::Deletion(line) => {
                let text = format!("- {line}");
                items.push(TokenItem {
                    value: text,
                    style: if colors_enabled {
                        super::token_item::TokenStyle::Magenta
                    } else {
                        super::token_item::TokenStyle::Raw
                    },
                });
            }
            Change::Noop(line) => {
                items.push(TokenItem {
                    value: format!("  {line}"),
                    style: super::token_item::TokenStyle::Raw,
                });
            }
        }
    }
    items
}

/// A token that renders a set of changes as a diff block.
pub struct LinesDiffContentToken {
    pub value: String,
    pub changes: Vec<Change>,
}

impl LinesDiffContentToken {
    pub fn new(value: String, changes: Vec<Change>) -> Self {
        Self { value, changes }
    }

    pub fn from_diff(value: String, old: &str, new: &str) -> Self {
        let changes = simple_diff(old, new);
        Self { value, changes }
    }

    pub fn render(&self, colors_enabled: bool) -> Vec<TokenItem> {
        let mut items = vec![TokenItem::raw(&self.value)];
        items.extend(render_lines_diff(&self.changes, colors_enabled));
        items
    }
}

/// A very simple line-by-line diff (no heuristic, no LCS).
/// Splits on newlines and compares line-by-line.
pub fn simple_diff(old: &str, new: &str) -> Vec<Change> {
    let old_lines: Vec<&str> = old.lines().collect();
    let new_lines: Vec<&str> = new.lines().collect();

    let max_len = old_lines.len().max(new_lines.len());
    let mut changes = Vec::with_capacity(max_len);

    for i in 0..max_len {
        match (old_lines.get(i), new_lines.get(i)) {
            (Some(old_line), Some(new_line)) if old_line == new_line => {
                changes.push(Change::Noop(new_line.to_string()));
            }
            (Some(old_line), Some(new_line)) => {
                changes.push(Change::Deletion(old_line.to_string()));
                changes.push(Change::Addition(new_line.to_string()));
            }
            (Some(old_line), None) => {
                changes.push(Change::Deletion(old_line.to_string()));
            }
            (None, Some(new_line)) => {
                changes.push(Change::Addition(new_line.to_string()));
            }
            (None, None) => {}
        }
    }

    changes
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_render_lines_diff_additions() {
        let changes = vec![Change::Addition("new line".into())];
        let items = render_lines_diff(&changes, true);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].render_plain(), "+ new line");
    }

    #[test]
    fn test_render_lines_diff_deletions() {
        let changes = vec![Change::Deletion("old line".into())];
        let items = render_lines_diff(&changes, true);
        assert_eq!(items[0].render_plain(), "- old line");
    }

    #[test]
    fn test_render_lines_diff_noop() {
        let changes = vec![Change::Noop("same".into())];
        let items = render_lines_diff(&changes, true);
        assert_eq!(items[0].render_plain(), "  same");
    }

    #[test]
    fn test_render_lines_diff_mixed() {
        let changes = vec![
            Change::Noop("keep".into()),
            Change::Deletion("remove".into()),
            Change::Addition("add".into()),
        ];
        let items = render_lines_diff(&changes, true);
        assert_eq!(items.len(), 3);
        assert_eq!(items[0].render_plain(), "  keep");
        assert_eq!(items[1].render_plain(), "- remove");
        assert_eq!(items[2].render_plain(), "+ add");
    }

    #[test]
    fn test_render_lines_diff_no_color() {
        let changes = vec![Change::Addition("line".into())];
        let items = render_lines_diff(&changes, false);
        assert_eq!(items[0].render_ansi(false), "+ line");
    }

    #[test]
    fn test_simple_diff_identical() {
        let changes = simple_diff("hello\nworld", "hello\nworld");
        assert_eq!(changes.len(), 2);
        assert_eq!(changes[0], Change::Noop("hello".into()));
        assert_eq!(changes[1], Change::Noop("world".into()));
    }

    #[test]
    fn test_simple_diff_added_line() {
        let changes = simple_diff("hello", "hello\nworld");
        assert_eq!(changes.len(), 2);
        assert!(changes.iter().any(|c| matches!(c, Change::Addition(_))));
    }

    #[test]
    fn test_simple_diff_removed_line() {
        let changes = simple_diff("hello\nworld", "hello");
        assert_eq!(changes.len(), 2);
        assert!(changes.iter().any(|c| matches!(c, Change::Deletion(_))));
    }

    #[test]
    fn test_simple_diff_changed_line() {
        let changes = simple_diff("hello", "world");
        assert_eq!(changes.len(), 2);
        assert!(changes.iter().any(|c| matches!(c, Change::Deletion(_))));
        assert!(changes.iter().any(|c| matches!(c, Change::Addition(_))));
    }

    #[test]
    fn test_lines_diff_token_render() {
        let token = LinesDiffContentToken::new("changes:".into(), vec![Change::Addition("add".into())]);
        let items = token.render(true);
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].render_plain(), "changes:");
        assert_eq!(items[1].render_plain(), "+ add");
    }
}
