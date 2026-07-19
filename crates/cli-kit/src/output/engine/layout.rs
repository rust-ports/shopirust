#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Layout {
    pub full_width: usize,
    pub two_thirds: usize,
    pub one_third: usize,
}

const MIN_FULL_WIDTH: usize = 20;
const MIN_FRACTION_WIDTH: usize = 80;

/// Calculate column widths based on terminal width.
/// Mirrors upstream `useLayout` + `calculateLayout`.
pub fn calculate_layout(terminal_width: usize) -> Layout {
    let full_width = terminal_width.max(MIN_FULL_WIDTH);

    let (one_third, two_thirds) = if full_width > MIN_FRACTION_WIDTH {
        (
            (full_width / 3).max(MIN_FRACTION_WIDTH),
            (full_width * 2 / 3).max(MIN_FRACTION_WIDTH),
        )
    } else {
        (full_width, full_width)
    };

    Layout {
        full_width,
        two_thirds,
        one_third,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_layout_small() {
        let layout = calculate_layout(40);
        assert_eq!(layout.full_width, 40);
        assert_eq!(layout.two_thirds, 40);
        assert_eq!(layout.one_third, 40);
    }

    #[test]
    fn test_calculate_layout_large() {
        let layout = calculate_layout(200);
        assert_eq!(layout.full_width, 200);
        assert!(layout.two_thirds >= 80);
        assert!(layout.one_third >= 80);
    }

    #[test]
    fn test_calculate_layout_minimum() {
        let layout = calculate_layout(10);
        assert_eq!(layout.full_width, 20);
    }

    #[test]
    fn test_calculate_layout_above_threshold() {
        let layout = calculate_layout(150);
        assert_eq!(layout.one_third, 80);
        assert_eq!(layout.two_thirds, 100);
    }

    #[test]
    fn test_calculate_layout_large_two_thirds() {
        let layout = calculate_layout(300);
        assert_eq!(layout.one_third, 100);
        assert_eq!(layout.two_thirds, 200);
    }

    #[test]
    fn test_layout_struct() {
        let layout = Layout {
            full_width: 120,
            two_thirds: 80,
            one_third: 40,
        };
        assert_eq!(layout.full_width, 120);
    }
}
