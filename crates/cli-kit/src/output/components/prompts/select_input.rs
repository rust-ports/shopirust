use crate::output::colors;
use crate::output::figures;

/// A single selectable option.
#[derive(Debug, Clone)]
pub struct Item<T> {
    pub label: String,
    pub value: T,
    pub hint: Option<String>,
    pub group: Option<String>,
    pub disabled: Option<String>,
}

impl<T> Item<T> {
    pub fn new(label: impl Into<String>, value: T) -> Self {
        Self {
            label: label.into(),
            value,
            hint: None,
            group: None,
            disabled: None,
        }
    }

    pub fn with_hint(mut self, hint: impl Into<String>) -> Self {
        self.hint = Some(hint.into());
        self
    }

    pub fn with_group(mut self, group: impl Into<String>) -> Self {
        self.group = Some(group.into());
        self
    }

    pub fn with_disabled(mut self, reason: impl Into<String>) -> Self {
        self.disabled = Some(reason.into());
        self
    }
}

/// Interactive select list with keyboard navigation, groups, and scrollbar.
#[derive(Debug, Clone)]
pub struct SelectInput<T> {
    items: Vec<Item<T>>,
    cursor: usize,
    page_size: usize,
}

impl<T> SelectInput<T> {
    pub fn new(items: Vec<Item<T>>) -> Self {
        let cursor = Self::first_selectable_index(&items);
        Self {
            items,
            cursor,
            page_size: 25,
        }
    }

    pub fn with_page_size(mut self, size: usize) -> Self {
        self.page_size = size.max(1);
        self
    }

    pub fn items(&self) -> &[Item<T>] {
        &self.items
    }

    pub fn cursor_index(&self) -> usize {
        self.cursor
    }

    pub fn total_items(&self) -> usize {
        self.items.len()
    }

    fn first_selectable_index(items: &[Item<T>]) -> usize {
        items.iter().position(|i| i.disabled.is_none()).unwrap_or(0)
    }

    fn last_selectable_index(items: &[Item<T>]) -> usize {
        items
            .iter()
            .rposition(|i| i.disabled.is_none())
            .unwrap_or(0)
    }

    fn next_selectable(&self, from: usize) -> Option<usize> {
        self.items[from + 1..]
            .iter()
            .position(|i| i.disabled.is_none())
            .map(|p| from + 1 + p)
    }

    fn prev_selectable(&self, from: usize) -> Option<usize> {
        self.items[..from]
            .iter()
            .rposition(|i| i.disabled.is_none())
    }

    pub fn cursor_up(&mut self) {
        if let Some(prev) = self.prev_selectable(self.cursor) {
            self.cursor = prev;
        }
    }

    pub fn cursor_down(&mut self) {
        if let Some(next) = self.next_selectable(self.cursor) {
            self.cursor = next;
        }
    }

    pub fn select_first(&mut self) {
        self.cursor = Self::first_selectable_index(&self.items);
    }

    pub fn select_last(&mut self) {
        self.cursor = Self::last_selectable_index(&self.items);
    }

    pub fn select_index(&mut self, index: usize) -> bool {
        if index < self.items.len() && self.items[index].disabled.is_none() {
            self.cursor = index;
            true
        } else {
            false
        }
    }

    pub fn page_up(&mut self) {
        for _ in 0..self.page_size.saturating_sub(1) {
            if self.prev_selectable(self.cursor).is_none() {
                break;
            }
            self.cursor_up();
        }
    }

    pub fn page_down(&mut self) {
        for _ in 0..self.page_size.saturating_sub(1) {
            if self.next_selectable(self.cursor).is_none() {
                break;
            }
            self.cursor_down();
        }
    }

    pub fn selected_value(&self) -> Option<T>
    where
        T: Clone,
    {
        self.items.get(self.cursor).map(|i| i.value.clone())
    }

    /// Visible range for pagination, consuming terminal height.
    pub fn visible_range(&self, available_lines: usize) -> (usize, usize) {
        let page = self.page_size.min(available_lines);
        if self.items.len() <= page {
            return (0, self.items.len());
        }
        let half = page / 2;
        let start = if self.cursor <= half {
            0
        } else if self.cursor + half >= self.items.len() {
            self.items.len().saturating_sub(page)
        } else {
            self.cursor.saturating_sub(half)
        };
        let end = (start + page).min(self.items.len());
        (start, end)
    }

    /// Build a group header line for a transition between items.
    fn group_header(group: &str, colors: bool) -> String {
        let sep = format!(
            "{} {} {}",
            figures::HORIZONTAL_LINE.repeat(6),
            group,
            figures::HORIZONTAL_LINE.repeat(6)
        );
        if colors {
            colors::dim(&sep)
        } else {
            sep
        }
    }

    fn render_item_prefix(index: usize, cursor: usize, colors: bool) -> String {
        if index == cursor {
            if colors {
                colors::cyan(figures::ARROW)
            } else {
                figures::ARROW.to_string()
            }
        } else {
            " ".repeat(2)
        }
    }

    /// Render items as ANSI lines, with group separators.
    pub fn render_items(&self, colors: bool) -> Vec<String> {
        let (start, end) = self.visible_range(usize::MAX);
        let mut lines = Vec::new();
        let mut prev_group: Option<String> = None;

        for i in start..end {
            let item = &self.items[i];

            // Group separator
            if item.group != prev_group {
                if let Some(ref g) = item.group {
                    lines.push(Self::group_header(g, colors));
                }
                prev_group = item.group.clone();
            }

            // Item line
            let prefix = Self::render_item_prefix(i, self.cursor, colors);
            let label = &item.label;
            let line = if let Some(ref reason) = item.disabled {
                let disabled_label = format!("{prefix}- {label}");
                let with_reason = if let Some(ref h) = item.hint {
                    format!("{disabled_label} ({reason}) {h}")
                } else {
                    format!("{disabled_label} ({reason})")
                };
                if colors {
                    colors::dim(&with_reason)
                } else {
                    with_reason
                }
            } else if let Some(ref h) = item.hint {
                let hint_str = if colors { colors::dim(h) } else { h.clone() };
                format!("{prefix}{label} {hint_str}")
            } else {
                format!("{prefix}{label}")
            };

            // Highlight active item
            let line = if i == self.cursor && colors {
                colors::cyan(&line)
            } else {
                line
            };

            lines.push(line);
        }

        lines
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_item_new() {
        let item = Item::new("Option A", 1);
        assert_eq!(item.label, "Option A");
        assert_eq!(item.value, 1);
    }

    #[test]
    fn test_item_with_hint() {
        let item = Item::new("Opt", "val").with_hint("hint text");
        assert_eq!(item.hint, Some("hint text".into()));
    }

    #[test]
    fn test_item_with_group() {
        let item = Item::new("Opt", 1).with_group("tools");
        assert_eq!(item.group, Some("tools".into()));
    }

    #[test]
    fn test_item_with_disabled() {
        let item = Item::new("Opt", 1).with_disabled("not available");
        assert_eq!(item.disabled, Some("not available".into()));
    }

    #[test]
    fn test_select_input_empty() {
        let input: SelectInput<i32> = SelectInput::new(vec![]);
        assert_eq!(input.total_items(), 0);
        assert_eq!(input.cursor_index(), 0);
    }

    #[test]
    fn test_select_input_single() {
        let input = SelectInput::new(vec![Item::new("only", 1)]);
        assert_eq!(input.cursor_index(), 0);
    }

    #[test]
    fn test_select_input_starts_on_first_selectable() {
        let items = vec![
            Item::new("one", 1).with_disabled("nope"),
            Item::new("two", 2),
        ];
        let input = SelectInput::new(items);
        assert_eq!(input.cursor_index(), 1);
    }

    #[test]
    fn test_select_input_navigation() {
        let mut input = SelectInput::new(vec![
            Item::new("one", 1),
            Item::new("two", 2),
            Item::new("three", 3),
        ]);
        assert_eq!(input.cursor_index(), 0);

        input.cursor_down();
        assert_eq!(input.cursor_index(), 1);

        input.cursor_down();
        assert_eq!(input.cursor_index(), 2);

        input.cursor_down();
        assert_eq!(input.cursor_index(), 2);

        input.cursor_up();
        assert_eq!(input.cursor_index(), 1);
    }

    #[test]
    fn test_select_input_skips_disabled() {
        let mut input = SelectInput::new(vec![
            Item::new("one", 1),
            Item::new("two", 2).with_disabled("skip"),
            Item::new("three", 3),
        ]);
        // cursor starts at 0 (first selectable)
        assert_eq!(input.cursor_index(), 0);

        input.cursor_down();
        // should skip index 1 (disabled), land on 2
        assert_eq!(input.cursor_index(), 2);
    }

    #[test]
    fn test_select_input_page_up_down() {
        let items: Vec<Item<usize>> = (0..20).map(|i| Item::new(format!("item {i}"), i)).collect();
        let mut input = SelectInput::new(items);
        input.page_down();
        assert!(input.cursor_index() > 0);
        input.select_first();
        assert_eq!(input.cursor_index(), 0);
    }

    #[test]
    fn test_select_input_select_first_last() {
        let mut input = SelectInput::new(vec![
            Item::new("a", 1),
            Item::new("b", 2),
            Item::new("c", 3),
        ]);
        input.select_last();
        assert_eq!(input.cursor_index(), 2);
        input.select_first();
        assert_eq!(input.cursor_index(), 0);
    }

    #[test]
    fn test_select_input_select_index() {
        let mut input = SelectInput::new(vec![Item::new("a", 1), Item::new("b", 2)]);
        assert!(input.select_index(1));
        assert_eq!(input.cursor_index(), 1);
        // can't select out of bounds
        assert!(!input.select_index(5));
    }

    #[test]
    fn test_select_input_selected_value() {
        let input = SelectInput::new(vec![Item::new("hello", "world")]);
        assert_eq!(input.selected_value(), Some("world"));
    }

    #[test]
    fn test_select_input_visible_range_small() {
        let items: Vec<Item<usize>> = (0..5).map(|i| Item::new(format!("i{i}"), i)).collect();
        let input = SelectInput::new(items);
        let (start, end) = input.visible_range(10);
        assert_eq!(start, 0);
        assert_eq!(end, 5);
    }

    #[test]
    fn test_select_input_render_items_with_arrow() {
        let input = SelectInput::new(vec![Item::new("alpha", 1)]);
        let lines = input.render_items(false);
        assert_eq!(lines.len(), 1);
        assert!(lines[0].contains(figures::ARROW));
    }

    #[test]
    fn test_select_input_render_items_groups() {
        let items = vec![
            Item::new("front", 1).with_group("UI"),
            Item::new("back", 2).with_group("API"),
        ];
        let input = SelectInput::new(items);
        let lines = input.render_items(false);
        // Should have 4 lines (2 group headers + 2 items)
        assert_eq!(lines.len(), 4);
        assert!(lines[0].contains(figures::HORIZONTAL_LINE));
        assert!(lines[2].contains(figures::HORIZONTAL_LINE));
    }

    #[test]
    fn test_select_input_disabled_render() {
        let input = SelectInput::new(vec![
            Item::new("good", 1),
            Item::new("bad", 2).with_disabled("broken"),
        ]);
        let lines = input.render_items(false);
        // Bad item should show disabled reason
        assert!(lines[1].contains("broken"));
    }
}
