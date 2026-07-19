/// State machine for interactive prompts.
/// Mirrors upstream `usePrompt` hook.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PromptState {
    Idle,
    Loading,
    Submitted,
    Error,
    Cancelled,
}

/// Mutable state for a prompt interaction.
#[derive(Debug, Clone)]
pub struct UsePrompt<T> {
    pub state: PromptState,
    pub answer: Option<T>,
    pub error: Option<String>,
}

impl<T> UsePrompt<T> {
    pub fn new() -> Self {
        Self {
            state: PromptState::Idle,
            answer: None,
            error: None,
        }
    }

    pub fn set_answer(&mut self, answer: T) {
        self.answer = Some(answer);
        self.state = PromptState::Submitted;
    }

    pub fn set_error(&mut self, error: String) {
        self.error = Some(error);
        self.state = PromptState::Error;
    }

    pub fn set_loading(&mut self) {
        self.state = PromptState::Loading;
    }

    pub fn cancel(&mut self) {
        self.state = PromptState::Cancelled;
    }

    pub fn is_done(&self) -> bool {
        matches!(self.state, PromptState::Submitted | PromptState::Cancelled)
    }
}

impl<T> Default for UsePrompt<T> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_use_prompt_default_idle() {
        let p: UsePrompt<String> = UsePrompt::new();
        assert_eq!(p.state, PromptState::Idle);
    }

    #[test]
    fn test_use_prompt_set_answer() {
        let mut p = UsePrompt::new();
        p.set_answer(42);
        assert_eq!(p.state, PromptState::Submitted);
        assert_eq!(p.answer, Some(42));
    }

    #[test]
    fn test_use_prompt_set_error() {
        let mut p: UsePrompt<String> = UsePrompt::new();
        p.set_error("bad input".into());
        assert_eq!(p.state, PromptState::Error);
        assert_eq!(p.error, Some("bad input".into()));
    }

    #[test]
    fn test_use_prompt_loading() {
        let mut p: UsePrompt<String> = UsePrompt::new();
        p.set_loading();
        assert_eq!(p.state, PromptState::Loading);
    }

    #[test]
    fn test_use_prompt_cancel() {
        let mut p: UsePrompt<String> = UsePrompt::new();
        p.cancel();
        assert_eq!(p.state, PromptState::Cancelled);
    }

    #[test]
    fn test_use_prompt_is_done() {
        let mut p: UsePrompt<String> = UsePrompt::new();
        assert!(!p.is_done());
        p.set_answer("done".into());
        assert!(p.is_done());
    }
}
