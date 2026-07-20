use crate::output::colors;
use crate::output::components::prompts::prompt_layout::PromptLayout;
use crate::output::components::prompts::select_input::{Item, SelectInput};
use crate::output::components::text_input::TextInput;
use crate::output::engine::{Event, EventResult, Prompt, RenderContext, RenderMode};
use crate::output::figures;
use crate::output::hooks::use_prompt::{PromptState, UsePrompt};
use std::fmt::Write as _;

/// Autocomplete prompt with search and selectable results.
/// Mirrors upstream `@clack/prompts` autocomplete function.
pub struct AutocompletePrompt {
    search_input: TextInput,
    select: SelectInput<String>,
    layout: PromptLayout,
    prompt: UsePrompt<String>,
    all_items: Vec<Item<String>>,
    is_filtered: bool,
}

impl AutocompletePrompt {
    /// Build an autocomplete prompt from all possible items.
    /// By default, all items are shown and typing filters by label (case-insensitive).
    pub fn new(message: impl Into<String>, items: Vec<Item<String>>) -> Self {
        let select = SelectInput::new(items.clone());
        Self {
            search_input: TextInput::new(""),
            select,
            layout: PromptLayout::new(message),
            prompt: UsePrompt::new(),
            all_items: items,
            is_filtered: false,
        }
    }

    fn apply_filter(&mut self) {
        let query = self.search_input.value().to_lowercase();
        if query.is_empty() {
            self.select = SelectInput::new(self.all_items.clone());
            self.is_filtered = false;
        } else {
            let filtered: Vec<Item<String>> = self
                .all_items
                .iter()
                .filter(|item| {
                    item.label.to_lowercase().contains(&query)
                        || item
                            .group
                            .as_deref()
                            .unwrap_or("")
                            .to_lowercase()
                            .contains(&query)
                })
                .cloned()
                .collect();
            self.select = SelectInput::new(filtered);
            self.is_filtered = true;
        }
    }
}

impl Prompt for AutocompletePrompt {
    type Value = String;

    fn render(&mut self, mode: &mut RenderMode, ctx: &RenderContext) {
        let colors = ctx.colors_enabled;
        let mut output = String::new();

        match self.prompt.state {
            PromptState::Submitted => {
                let answer = self.prompt.answer.as_deref().unwrap_or("");
                let prefix = if colors {
                    colors::green(figures::TICK)
                } else {
                    figures::TICK.to_string()
                };
                let _ = writeln!(
                    output,
                    "{prefix} {} {}",
                    self.layout.message,
                    colors::cyan(answer)
                );
            }
            PromptState::Cancelled => {
                let prefix = if colors {
                    colors::red(figures::CROSS)
                } else {
                    figures::CROSS.to_string()
                };
                let _ = writeln!(output, "{prefix} {} (cancelled)", self.layout.message);
            }
            _ => {
                let prefix = if colors {
                    colors::cyan("?")
                } else {
                    "?".to_string()
                };
                let _ = writeln!(output, "{prefix} {}", self.layout.message);

                // Search input line
                let search_input_render = self.search_input.render_ansi();
                let _ = writeln!(
                    output,
                    "{} {}",
                    colors::cyan(figures::ARROW),
                    search_input_render
                );

                // Results
                if self.select.total_items() == 0 {
                    if self.is_filtered {
                        let _ = writeln!(output, "  {}", colors::dim("No matches found"));
                    } else if !self.all_items.is_empty() {
                        let _ = writeln!(output, "  {}", colors::dim("Start typing to search"));
                    } else {
                        let _ = writeln!(output, "  {}", colors::dim("No items available"));
                    }
                } else {
                    let available =
                        PromptLayout::available_lines(ctx.terminal_height).saturating_sub(3);
                    let rendered = self.select.render_items(colors);
                    let (start, end) = self.select.visible_range(available);
                    for line in &rendered[start..end] {
                        let _ = writeln!(output, "{line}");
                    }
                }
            }
        }

        match mode {
            RenderMode::Ansi(buf) => {
                let _ = write!(buf, "{output}");
            }
            RenderMode::Tui(_frame) => {}
        }
    }

    fn handle_event(&mut self, event: &Event) -> EventResult<Self::Value> {
        if self.prompt.is_done() {
            return EventResult::Continue;
        }

        match event {
            Event::Key(key) => match key.code {
                crossterm::event::KeyCode::Char(ch)
                    if key.modifiers.is_empty()
                        || key.modifiers == crossterm::event::KeyModifiers::SHIFT =>
                {
                    self.search_input.insert(ch);
                    self.apply_filter();
                    EventResult::Continue
                }
                crossterm::event::KeyCode::Backspace => {
                    self.search_input.delete_before();
                    self.apply_filter();
                    EventResult::Continue
                }
                crossterm::event::KeyCode::Delete => {
                    self.search_input.delete_after();
                    self.apply_filter();
                    EventResult::Continue
                }
                crossterm::event::KeyCode::Left => {
                    self.search_input.cursor_left();
                    EventResult::Continue
                }
                crossterm::event::KeyCode::Right => {
                    self.search_input.cursor_right();
                    EventResult::Continue
                }
                crossterm::event::KeyCode::Home => {
                    self.search_input.cursor_home();
                    EventResult::Continue
                }
                crossterm::event::KeyCode::End => {
                    self.search_input.cursor_end();
                    EventResult::Continue
                }
                crossterm::event::KeyCode::Up | crossterm::event::KeyCode::Char('k') => {
                    self.select.cursor_up();
                    EventResult::Continue
                }
                crossterm::event::KeyCode::Down | crossterm::event::KeyCode::Char('j') => {
                    self.select.cursor_down();
                    EventResult::Continue
                }
                crossterm::event::KeyCode::PageUp => {
                    self.select.page_up();
                    EventResult::Continue
                }
                crossterm::event::KeyCode::PageDown => {
                    self.select.page_down();
                    EventResult::Continue
                }
                crossterm::event::KeyCode::Tab => {
                    // Tab-fill: autocomplete with selected item's value
                    if let Some(value) = self.select.selected_value() {
                        if !value.is_empty() {
                            self.search_input.set_value(value);
                            self.apply_filter();
                        }
                    }
                    EventResult::Continue
                }
                crossterm::event::KeyCode::Enter => {
                    let value = self.search_input.value().to_string();
                    if value.is_empty() {
                        return EventResult::Continue;
                    }
                    self.prompt.set_answer(value.clone());
                    EventResult::Submit(value)
                }
                crossterm::event::KeyCode::Esc => {
                    self.prompt.cancel();
                    EventResult::Cancel
                }
                crossterm::event::KeyCode::Char('c')
                    if key.modifiers == crossterm::event::KeyModifiers::CONTROL =>
                {
                    self.prompt.cancel();
                    EventResult::Cancel
                }
                _ => EventResult::Continue,
            },
            Event::Resize(_, _) => EventResult::Continue,
            Event::Paste(s) => {
                self.search_input.insert_str(s);
                self.apply_filter();
                EventResult::Continue
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_items() -> Vec<Item<String>> {
        vec![
            Item::new("apple", "apple".into()).with_group("fruit"),
            Item::new("banana", "banana".into()).with_group("fruit"),
            Item::new("carrot", "carrot".into()).with_group("veggie"),
            Item::new("date", "date".into()).with_group("fruit"),
        ]
    }

    #[test]
    fn test_autocomplete_new() {
        let prompt = AutocompletePrompt::new("search", sample_items());
        assert_eq!(prompt.select.total_items(), 4);
    }

    #[test]
    fn test_autocomplete_filter() {
        let mut prompt = AutocompletePrompt::new("search", sample_items());

        // Type "app"
        for ch in "app".chars() {
            prompt.handle_event(&Event::Key(crossterm::event::KeyEvent::new(
                crossterm::event::KeyCode::Char(ch),
                crossterm::event::KeyModifiers::NONE,
            )));
        }

        // After filtering, only "apple" should remain
        assert_eq!(prompt.select.total_items(), 1);
        assert_eq!(prompt.select.items()[0].label, "apple");
    }

    #[test]
    fn test_autocomplete_filter_empty_restores_all() {
        let mut prompt = AutocompletePrompt::new("search", sample_items());

        // Type and clear
        for ch in "a".chars() {
            prompt.handle_event(&Event::Key(crossterm::event::KeyEvent::new(
                crossterm::event::KeyCode::Char(ch),
                crossterm::event::KeyModifiers::NONE,
            )));
        }
        prompt.handle_event(&Event::Key(crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::Backspace,
            crossterm::event::KeyModifiers::NONE,
        )));

        assert_eq!(prompt.select.total_items(), 4);
    }

    #[test]
    fn test_autocomplete_submit() {
        let mut prompt = AutocompletePrompt::new("search", sample_items());

        // Type a value
        for ch in "banana".chars() {
            prompt.handle_event(&Event::Key(crossterm::event::KeyEvent::new(
                crossterm::event::KeyCode::Char(ch),
                crossterm::event::KeyModifiers::NONE,
            )));
        }

        let result = prompt.handle_event(&Event::Key(crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::Enter,
            crossterm::event::KeyModifiers::NONE,
        )));
        match result {
            EventResult::Submit(v) => assert_eq!(v, "banana"),
            _ => panic!("expected Submit"),
        }
    }

    #[test]
    fn test_autocomplete_cancel() {
        let mut prompt = AutocompletePrompt::new("s", sample_items());
        let result = prompt.handle_event(&Event::Key(crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::Esc,
            crossterm::event::KeyModifiers::NONE,
        )));
        assert!(matches!(result, EventResult::Cancel));
    }

    #[test]
    fn test_autocomplete_tab_fill() {
        let mut prompt = AutocompletePrompt::new("s", sample_items());

        // Type "car" then tab should fill with "carrot"
        for ch in "car".chars() {
            prompt.handle_event(&Event::Key(crossterm::event::KeyEvent::new(
                crossterm::event::KeyCode::Char(ch),
                crossterm::event::KeyModifiers::NONE,
            )));
        }

        prompt.handle_event(&Event::Key(crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::Tab,
            crossterm::event::KeyModifiers::NONE,
        )));

        assert_eq!(prompt.search_input.value(), "carrot");
    }

    #[test]
    fn test_autocomplete_arrow_navigation() {
        let mut prompt = AutocompletePrompt::new("s", sample_items());

        // We start with all items, cursor at first
        assert_eq!(prompt.select.cursor_index(), 0);

        prompt.handle_event(&Event::Key(crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::Down,
            crossterm::event::KeyModifiers::NONE,
        )));

        assert_eq!(prompt.select.cursor_index(), 1);
    }

    #[test]
    fn test_autocomplete_render_idle() {
        let mut prompt = AutocompletePrompt::new("find", sample_items());
        let ctx = RenderContext::default();
        let mut buf = String::new();
        prompt.render(&mut RenderMode::Ansi(&mut buf), &ctx);
        assert!(buf.contains("find"));
    }

    #[test]
    fn test_autocomplete_render_submitted() {
        let mut prompt = AutocompletePrompt::new("q", sample_items());
        for ch in "date".chars() {
            prompt.handle_event(&Event::Key(crossterm::event::KeyEvent::new(
                crossterm::event::KeyCode::Char(ch),
                crossterm::event::KeyModifiers::NONE,
            )));
        }
        let _ = prompt.handle_event(&Event::Key(crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::Enter,
            crossterm::event::KeyModifiers::NONE,
        )));
        let ctx = RenderContext::default();
        let mut buf = String::new();
        prompt.render(&mut RenderMode::Ansi(&mut buf), &ctx);
        assert!(buf.contains(figures::TICK));
    }

    #[test]
    fn test_autocomplete_empty_state() {
        let prompt = AutocompletePrompt::new("s", vec![]);
        let ctx = RenderContext::default();
        let mut buf = String::new();
        // Can't render mutably without mut, use a workaround
        let mut p = prompt;
        p.render(&mut RenderMode::Ansi(&mut buf), &ctx);
        assert!(buf.contains("No items available"));
    }
}
