/// Ctrl+C handler hook — tracks whether the user pressed Ctrl+C.
/// Mirrors upstream `useExitOnCtrlC` hook.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct UseExitOnCtrlC {
    pub triggered: bool,
}

impl UseExitOnCtrlC {
    pub fn new() -> Self {
        Self { triggered: false }
    }

    /// Call this when a Ctrl+C event is received.
    pub fn trigger(&mut self) {
        self.triggered = true;
    }

    pub fn reset(&mut self) {
        self.triggered = false;
    }

    pub fn is_triggered(&self) -> bool {
        self.triggered
    }

    /// Create a Ctrl+C handler function that sets the triggered flag.
    pub fn handler() -> impl FnMut(crossterm::event::KeyEvent) -> bool {
        |key: crossterm::event::KeyEvent| {
            key.code == crossterm::event::KeyCode::Char('c')
                && key.modifiers == crossterm::event::KeyModifiers::CONTROL
        }
    }
}

impl Default for UseExitOnCtrlC {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_exit_on_ctrl_c_default() {
        let h = UseExitOnCtrlC::new();
        assert!(!h.is_triggered());
    }

    #[test]
    fn test_exit_on_ctrl_c_trigger() {
        let mut h = UseExitOnCtrlC::new();
        h.trigger();
        assert!(h.is_triggered());
    }

    #[test]
    fn test_exit_on_ctrl_c_reset() {
        let mut h = UseExitOnCtrlC::new();
        h.trigger();
        h.reset();
        assert!(!h.is_triggered());
    }

    #[test]
    fn test_ctrl_c_handler_match() {
        let mut handler = UseExitOnCtrlC::handler();
        let ctrl_c = crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::Char('c'),
            crossterm::event::KeyModifiers::CONTROL,
        );
        assert!(handler(ctrl_c));
    }

    #[test]
    fn test_ctrl_c_handler_no_match() {
        let mut handler = UseExitOnCtrlC::handler();
        let key = crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::Char('q'),
            crossterm::event::KeyModifiers::NONE,
        );
        assert!(!handler(key));
    }
}
