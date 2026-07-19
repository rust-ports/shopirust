use crate::output::figures;

/// Visual scrollbar: `│` background, `║` position, `△`/`▽` arrows.
#[derive(Debug, Clone)]
pub struct Scrollbar {
    pub total: usize,
    pub visible: usize,
    pub position: usize,
}

impl Scrollbar {
    pub fn new(total: usize, visible: usize, position: usize) -> Self {
        Self {
            total,
            visible,
            position,
        }
    }

    /// Whether scrolling is needed at all.
    pub fn is_needed(&self) -> bool {
        self.total > self.visible
    }

    /// The scrollbar position as a fraction of the total height (0.0 to 1.0).
    pub fn scroll_fraction(&self) -> f64 {
        if self.total == 0 {
            return 0.0;
        }
        let max_scroll = self.total.saturating_sub(self.visible);
        if max_scroll == 0 {
            return 0.0;
        }
        self.position as f64 / max_scroll as f64
    }

    /// Render the scrollbar as a single-line string.
    /// Returns empty string if no scrolling is needed.
    pub fn render(&self, width: usize) -> String {
        if !self.is_needed() || width < 2 {
            return String::new();
        }

        let scrollbar_height = width.saturating_sub(2);
        let thumb_pos = (self.scroll_fraction() * scrollbar_height as f64).round() as usize;
        let thumb_pos = thumb_pos.min(scrollbar_height.saturating_sub(1));

        let mut bar = String::with_capacity(width);
        bar.push_str(figures::TRIANGLE_UP);

        for i in 0..scrollbar_height {
            if i == thumb_pos {
                bar.push_str(figures::SCROLL_BAR);
            } else {
                bar.push_str(figures::VERTICAL_BAR);
            }
        }

        bar.push_str(figures::TRIANGLE_DOWN);
        bar
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scrollbar_not_needed() {
        let sb = Scrollbar::new(5, 10, 0);
        assert!(!sb.is_needed());
    }

    #[test]
    fn test_scrollbar_needed() {
        let sb = Scrollbar::new(10, 5, 0);
        assert!(sb.is_needed());
    }

    #[test]
    fn test_scrollbar_fraction_zero() {
        let sb = Scrollbar::new(0, 0, 0);
        assert_eq!(sb.scroll_fraction(), 0.0);
    }

    #[test]
    fn test_scrollbar_fraction_start() {
        let sb = Scrollbar::new(10, 5, 0);
        assert_eq!(sb.scroll_fraction(), 0.0);
    }

    #[test]
    fn test_scrollbar_fraction_end() {
        let sb = Scrollbar::new(10, 5, 5);
        assert!((sb.scroll_fraction() - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_scrollbar_render_empty() {
        let sb = Scrollbar::new(3, 10, 0);
        assert_eq!(sb.render(5), "");
    }

    #[test]
    fn test_scrollbar_render_basic() {
        let sb = Scrollbar::new(10, 3, 0);
        let rendered = sb.render(6);
        assert!(!rendered.is_empty());
        assert!(rendered.starts_with(figures::TRIANGLE_UP));
        assert!(rendered.ends_with(figures::TRIANGLE_DOWN));
    }

    #[test]
    fn test_scrollbar_render_small_width() {
        let sb = Scrollbar::new(10, 3, 0);
        assert_eq!(sb.render(1), "");
    }
}
