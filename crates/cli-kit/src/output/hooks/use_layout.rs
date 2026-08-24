use crate::output::engine::layout::{calculate_layout, Layout};

/// Reactive layout hook — tracks terminal dimensions.
/// Mirrors upstream `useLayout` hook.
#[derive(Debug, Clone, Copy)]
pub struct UseLayout {
    pub layout: Layout,
    pub terminal_width: usize,
    pub terminal_height: usize,
}

impl UseLayout {
    pub fn new(terminal_width: usize, terminal_height: usize) -> Self {
        let layout = calculate_layout(terminal_width);
        Self {
            layout,
            terminal_width,
            terminal_height,
        }
    }

    pub fn resize(&mut self, width: usize, height: usize) {
        self.terminal_width = width;
        self.terminal_height = height;
        self.layout = calculate_layout(width);
    }
}

impl Default for UseLayout {
    fn default() -> Self {
        Self::new(80, 24)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_use_layout_default() {
        let ul = UseLayout::default();
        assert_eq!(ul.terminal_width, 80);
        assert_eq!(ul.terminal_height, 24);
    }

    #[test]
    fn test_use_layout_custom() {
        let ul = UseLayout::new(120, 30);
        assert_eq!(ul.terminal_width, 120);
        assert_eq!(ul.terminal_height, 30);
    }

    #[test]
    fn test_use_layout_resize() {
        let mut ul = UseLayout::new(80, 24);
        ul.resize(200, 40);
        assert_eq!(ul.terminal_width, 200);
        assert_eq!(ul.terminal_height, 40);
    }

    #[test]
    fn test_use_layout_has_full_width() {
        let ul = UseLayout::new(100, 24);
        assert_eq!(ul.layout.full_width, 100);
    }
}
