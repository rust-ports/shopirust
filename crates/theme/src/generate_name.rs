use crate::replace_invalid_chars::replace_invalid_characters;
use rand::seq::SliceRandom;
use rand::Rng;

pub const API_NAME_LIMIT: usize = 50;

const CREATIVE_ADJECTIVES: &[&str] = &[
    "bright",
    "impactful",
    "stylish",
    "colorful",
    "modern",
    "minimal",
    "trendy",
    "creative",
    "artistic",
    "spectacular",
    "glamorous",
    "luxury",
    "retro",
    "nostalgic",
    "comfy",
    "polished",
    "fabulous",
    "balanced",
    "monochrome",
    "glitched",
    "contrasted",
    "elegant",
    "textured",
    "vibrant",
    "harmonious",
    "versatile",
    "eclectic",
    "futuristic",
    "idealistic",
    "intricate",
    "bohemian",
    "abstract",
    "meticulous",
    "refined",
    "flamboyant",
];

const CREATIVE_NOUNS: &[&str] = &[
    "vibe",
    "style",
    "moment",
    "mood",
    "flavor",
    "look",
    "appearance",
    "perspective",
    "aspect",
    "ambience",
    "quality",
    "backdrop",
    "focus",
    "tone",
    "inspiration",
    "imagery",
    "aesthetics",
    "palette",
    "ornamentation",
    "contrast",
    "colorway",
    "visuals",
    "typography",
    "composition",
    "scale",
    "symmetry",
    "gradients",
    "proportions",
    "textures",
    "harmony",
    "shapes",
    "patterns",
];

pub fn generate_creative_name() -> String {
    let mut rng = rand::thread_rng();
    format!(
        "{}-{}",
        CREATIVE_ADJECTIVES
            .choose(&mut rng)
            .expect("creative adjectives"),
        CREATIVE_NOUNS.choose(&mut rng).expect("creative nouns")
    )
}

pub fn generate_theme_name(context: &str) -> String {
    let hostname = std::env::var("HOSTNAME")
        .or_else(|_| std::env::var("hostname"))
        .unwrap_or_else(|_| "localhost".to_string());
    let hostname_no_domain = hostname.split('.').next().unwrap_or(&hostname).to_string();
    let hash = random_hex(3);
    let base = format!("{context} ()");
    let hostname_char_limit = API_NAME_LIMIT
        .saturating_sub(base.len())
        .saturating_sub(hash.len())
        .saturating_sub(1);
    let truncated = &hostname_no_domain[..hostname_no_domain.len().min(hostname_char_limit)];
    let identifier = replace_invalid_characters(&format!("{hash}-{truncated}"));
    format!("{context} ({identifier})")
}

fn random_hex(bytes: usize) -> String {
    let mut rng = rand::thread_rng();
    (0..bytes)
        .map(|_| format!("{:02x}", rng.gen::<u8>()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generates_name_with_context() {
        let name = generate_theme_name("Development");
        assert!(name.starts_with("Development ("));
        assert!(name.ends_with(')'));
    }

    #[test]
    fn test_generates_name_within_length() {
        let name = generate_theme_name("Development");
        assert!(name.len() <= API_NAME_LIMIT, "name too long: {name}");
    }

    #[test]
    fn test_different_contexts() {
        let dev = generate_theme_name("Development");
        let staging = generate_theme_name("Staging");
        assert_ne!(dev, staging);
    }

    #[test]
    fn creative_name_matches_upstream_shape() {
        let name = generate_creative_name();
        let (adjective, noun) = name.split_once('-').unwrap();
        assert!(CREATIVE_ADJECTIVES.contains(&adjective));
        assert!(CREATIVE_NOUNS.contains(&noun));
    }
}
