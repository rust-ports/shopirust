/// Rainbow HSV gradient animation, styled after upstream's `gradient-string`.
/// Cycles hue every `FRAME_MS` (35ms default) for a smooth rainbow effect.
#[derive(Debug, Clone)]
pub struct TextAnimation {
    hue: f64,
    frame_count: u64,
}

impl Default for TextAnimation {
    fn default() -> Self {
        Self::new()
    }
}

impl TextAnimation {
    pub fn new() -> Self {
        Self {
            hue: 0.0,
            frame_count: 0,
        }
    }

    /// Advance the animation by one frame (35ms).
    pub fn tick(&mut self) {
        self.hue = (self.hue + 5.0) % 360.0;
        self.frame_count += 1;
    }

    /// Current hue value in degrees (0-360).
    pub fn current_hue(&self) -> f64 {
        self.hue
    }

    /// Frame number since creation.
    pub fn frame_count(&self) -> u64 {
        self.frame_count
    }

    /// Apply a rainbow gradient to the given text.
    /// Each character gets a slightly shifted hue for a smooth gradient effect.
    pub fn apply_rainbow(&self, text: &str) -> String {
        let chars: Vec<char> = text.chars().collect();
        if chars.is_empty() {
            return String::new();
        }

        let hue_step = 30.0;
        let mut result = String::with_capacity(text.len() * 20);

        for (i, c) in chars.iter().enumerate() {
            let char_hue = (self.hue + i as f64 * hue_step) % 360.0;
            let (r, g, b) = hsv_to_rgb(char_hue, 0.8, 1.0);
            result.push_str(&format!("\x1b[38;2;{r};{g};{b}m{c}\x1b[0m"));
        }

        result
    }
}

/// Convert HSV to RGB (all values 0-255).
fn hsv_to_rgb(h: f64, s: f64, v: f64) -> (u8, u8, u8) {
    let c = v * s;
    let x = c * (1.0 - ((h / 60.0) % 2.0 - 1.0).abs());
    let m = v - c;

    let (r1, g1, b1) = if h < 60.0 {
        (c, x, 0.0)
    } else if h < 120.0 {
        (x, c, 0.0)
    } else if h < 180.0 {
        (0.0, c, x)
    } else if h < 240.0 {
        (0.0, x, c)
    } else if h < 300.0 {
        (x, 0.0, c)
    } else {
        (c, 0.0, x)
    };

    (
        ((r1 + m) * 255.0) as u8,
        ((g1 + m) * 255.0) as u8,
        ((b1 + m) * 255.0) as u8,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_text_animation_default_hue() {
        let anim = TextAnimation::new();
        assert_eq!(anim.current_hue(), 0.0);
    }

    #[test]
    fn test_text_animation_tick() {
        let mut anim = TextAnimation::new();
        anim.tick();
        assert_eq!(anim.current_hue(), 5.0);
        assert_eq!(anim.frame_count(), 1);
    }

    #[test]
    fn test_text_animation_wraps_hue() {
        let mut anim = TextAnimation::new();
        for _ in 0..73 {
            anim.tick();
        }
        assert!((anim.current_hue() - 5.0).abs() < 0.01);
        assert_eq!(anim.frame_count(), 73);
    }

    #[test]
    fn test_apply_rainbow_empty() {
        let anim = TextAnimation::new();
        assert_eq!(anim.apply_rainbow(""), "");
    }

    #[test]
    fn test_apply_rainbow_produces_ansi() {
        let anim = TextAnimation::new();
        let result = anim.apply_rainbow("hello");
        assert!(result.starts_with("\x1b[38;2;"));
        // Each character is individually wrapped, so "hello" isn't contiguous
        for ch in "hello".chars() {
            assert!(result.contains(ch), "missing char {ch} in output");
        }
    }

    #[test]
    fn test_apply_rainbow_all_chars_colored() {
        let anim = TextAnimation::new();
        let result = anim.apply_rainbow("ab");
        let segments: Vec<&str> = result.split("\x1b[0m").collect();
        assert!(segments.len() >= 2);
    }

    #[test]
    fn test_hsv_to_rgb_red() {
        let (r, g, b) = hsv_to_rgb(0.0, 1.0, 1.0);
        assert_eq!(r, 255);
        assert_eq!(g, 0);
        assert_eq!(b, 0);
    }

    #[test]
    fn test_hsv_to_rgb_green() {
        let (r, g, b) = hsv_to_rgb(120.0, 1.0, 1.0);
        assert_eq!(r, 0);
        assert_eq!(g, 255);
        assert_eq!(b, 0);
    }

    #[test]
    fn test_hsv_to_rgb_blue() {
        let (r, g, b) = hsv_to_rgb(240.0, 1.0, 1.0);
        assert_eq!(r, 0);
        assert_eq!(g, 0);
        assert_eq!(b, 255);
    }
}
