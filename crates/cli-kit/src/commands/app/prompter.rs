//! CLI-kit adapter that renders app prompts via `output::public_api`.

use app::error::AppError;
use app::prompts::{PromptItem, Prompter};
use crate::output::components::prompts::select_input::Item;
use crate::output::public_api::{
    render_autocomplete_prompt, render_confirmation_prompt, render_dangerous_confirmation_prompt,
    render_select_prompt, render_text_prompt,
};

#[derive(Debug, Default, Clone, Copy)]
pub struct CliKitPrompter;

impl Prompter for CliKitPrompter {
    fn select(&self, message: &str, items: &[PromptItem]) -> Result<String, AppError> {
        let mapped: Vec<Item<String>> = items
            .iter()
            .map(|i| {
                let mut item = Item::new(i.label.clone(), i.value.clone());
                if let Some(ref hint) = i.hint {
                    item = item.with_hint(hint.clone());
                }
                if let Some(ref group) = i.group {
                    item = item.with_group(group.clone());
                }
                item
            })
            .collect();
        render_select_prompt(message, mapped).map_err(AppError::message)
    }

    fn autocomplete(&self, message: &str, items: &[PromptItem]) -> Result<String, AppError> {
        let mapped: Vec<Item<String>> = items
            .iter()
            .map(|i| Item::new(i.label.clone(), i.value.clone()))
            .collect();
        render_autocomplete_prompt(message, mapped).map_err(AppError::message)
    }

    fn confirm(&self, message: &str) -> Result<bool, AppError> {
        render_confirmation_prompt(message).map_err(AppError::message)
    }

    fn text(&self, message: &str, _initial: Option<&str>) -> Result<String, AppError> {
        render_text_prompt(message).map_err(AppError::message)
    }

    fn dangerous_confirm(&self, message: &str, confirmation_word: &str) -> Result<bool, AppError> {
        render_dangerous_confirmation_prompt(message, confirmation_word).map_err(AppError::message)
    }
}
