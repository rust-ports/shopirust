use crate::commands::search::searchable_commands;

/// Suggest the closest known command for an unknown argv token.
pub fn did_you_mean(input: &str) -> Option<String> {
    let input = input.to_lowercase();
    let mut best: Option<(String, f64)> = None;
    for cmd in searchable_commands() {
        let score = jaro_winkler(&input, &cmd);
        let leaf = cmd.split_whitespace().last().unwrap_or(&cmd);
        let leaf_score = jaro_winkler(&input, leaf);
        let score = score.max(leaf_score);
        if score > 0.75 && best.as_ref().map(|(_, s)| score > *s).unwrap_or(true) {
            best = Some((cmd, score));
        }
    }
    best.map(|(c, _)| c)
}

fn jaro_winkler(a: &str, b: &str) -> f64 {
    strsim::jaro_winkler(a, b)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn suggests_deploy() {
        assert_eq!(did_you_mean("deply"), Some("app deploy".into()));
    }

    #[test]
    fn none_for_garbage() {
        assert!(did_you_mean("zzzzzzzz").is_none());
    }
}
