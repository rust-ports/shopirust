//! Unicode symbols used throughout the UI.
//! Mirrors upstream `figures.ts`.

/// Checkmark for success states.
pub const TICK: &str = "✔";

/// Cross mark for failure/error states.
pub const CROSS: &str = "✖";

/// Bullet point for unordered lists.
pub const BULLET: &str = "•";

/// Diamond for section markers.
pub const DIAMOND: &str = "◆";

/// Double vertical bar (used for thin borders).
pub const DOUBLE_VERTICAL: &str = "‖";

/// Horizontal line segment.
pub const HORIZONTAL_LINE: &str = "─";

/// Up-pointing triangle (scroll up indicator).
pub const TRIANGLE_UP: &str = "△";

/// Down-pointing triangle (scroll down indicator).
pub const TRIANGLE_DOWN: &str = "▽";

/// Filled square.
pub const SQUARE: &str = "■";

/// Upper horizontal line.
pub const UPPER_LINE: &str = "▔";

/// Single vertical bar.
pub const VERTICAL_BAR: &str = "│";

/// Double vertical bar for scrollbar handle.
pub const SCROLL_BAR: &str = "║";

/// Filled circle (selection indicator).
pub const SELECTED: &str = "◉";

/// Arrow for directional indicators.
pub const ARROW: &str = "→";

/// Return the appropriate bullet symbol based on whether the list is ordered.
pub fn list_bullet(index: usize, ordered: bool) -> String {
    if ordered {
        format!("{}. ", index + 1)
    } else {
        format!("{BULLET} ")
    }
}

/// Return the success/fail symbol.
pub fn status_icon(success: bool) -> &'static str {
    if success {
        TICK
    } else {
        CROSS
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_figures_are_non_empty() {
        assert!(!TICK.is_empty());
        assert!(!CROSS.is_empty());
        assert!(!BULLET.is_empty());
        assert!(!DIAMOND.is_empty());
        assert!(!DOUBLE_VERTICAL.is_empty());
        assert!(!HORIZONTAL_LINE.is_empty());
        assert!(!TRIANGLE_UP.is_empty());
        assert!(!TRIANGLE_DOWN.is_empty());
        assert!(!SQUARE.is_empty());
        assert!(!UPPER_LINE.is_empty());
        assert!(!VERTICAL_BAR.is_empty());
        assert!(!SCROLL_BAR.is_empty());
        assert!(!SELECTED.is_empty());
        assert!(!ARROW.is_empty());
    }

    #[test]
    fn test_list_bullet_ordered() {
        assert_eq!(list_bullet(0, true), "1. ");
        assert_eq!(list_bullet(2, true), "3. ");
    }

    #[test]
    fn test_list_bullet_unordered() {
        assert_eq!(list_bullet(0, false), "• ");
    }

    #[test]
    fn test_status_icon_success() {
        assert_eq!(status_icon(true), TICK);
    }

    #[test]
    fn test_status_icon_failure() {
        assert_eq!(status_icon(false), CROSS);
    }
}
