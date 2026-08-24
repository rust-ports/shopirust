//! Rank Zod-style union errors the same way upstream `error-parsing.ts` does.

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ParsedIssue {
    pub path: Option<Vec<Value>>,
    pub message: String,
    pub code: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct UnionError {
    #[serde(default)]
    issues: Option<Vec<InnerIssue>>,
    #[serde(default)]
    #[allow(dead_code)]
    name: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct InnerIssue {
    path: Option<Vec<Value>>,
    message: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct ExtendedIssue {
    path: Option<Vec<Value>>,
    message: Option<String>,
    code: Option<String>,
    #[serde(rename = "unionErrors")]
    union_errors: Option<Vec<UnionError>>,
}

/// Flatten structured Zod issues, picking the best matching union variant.
pub fn parse_structured_errors(issues: &[Value]) -> Vec<ParsedIssue> {
    issues
        .iter()
        .filter_map(|raw| serde_json::from_value::<ExtendedIssue>(raw.clone()).ok())
        .flat_map(|issue| {
            if issue.code.as_deref() == Some("invalid_union") {
                if let Some(ref unions) = issue.union_errors {
                    if let Some(best) = find_best_matching_variant(unions) {
                        return best
                            .issues
                            .clone()
                            .unwrap_or_default()
                            .into_iter()
                            .map(|inner| ParsedIssue {
                                path: inner.path.or_else(|| issue.path.clone()),
                                message: inner.message.unwrap_or_else(|| "Unknown error".into()),
                                code: issue.code.clone(),
                            })
                            .collect::<Vec<_>>();
                    }
                }
                return vec![ParsedIssue {
                    path: issue.path,
                    message: issue.message.unwrap_or_else(|| {
                        "Configuration doesn't match any expected format".into()
                    }),
                    code: issue.code,
                }];
            }
            vec![ParsedIssue {
                path: issue.path,
                message: issue.message.unwrap_or_else(|| "Unknown error".into()),
                code: issue.code,
            }]
        })
        .collect()
}

fn find_best_matching_variant(union_errors: &[UnionError]) -> Option<&UnionError> {
    union_errors
        .iter()
        .filter(|u| u.issues.as_ref().is_some_and(|i| !i.is_empty()))
        .max_by_key(|u| score_variant(u.issues.as_deref().unwrap_or(&[])))
}

fn score_variant(issues: &[InnerIssue]) -> i32 {
    let msgs: Vec<&str> = issues.iter().filter_map(|i| i.message.as_deref()).collect();
    let required = msgs
        .iter()
        .filter(|m| m.contains("Required") || m.contains("required"))
        .count() as i32;
    let expected = msgs
        .iter()
        .filter(|m| m.contains("Expected") && m.contains("received"))
        .count() as i32;
    let other = msgs.len() as i32 - required - expected;
    if required > 0 {
        1000 - required * 10 - expected - other
    } else if expected > 0 {
        100 - expected * 5 - other
    } else {
        50 - other
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn passthrough_simple_issue() {
        let parsed = parse_structured_errors(&[json!({
            "path": ["name"],
            "message": "Required",
            "code": "invalid_type"
        })]);
        assert_eq!(parsed[0].message, "Required");
        assert_eq!(parsed[0].code.as_deref(), Some("invalid_type"));
    }

    #[test]
    fn prefers_required_variant() {
        let parsed = parse_structured_errors(&[json!({
            "code": "invalid_union",
            "unionErrors": [
                { "name": "a", "issues": [{ "message": "Expected string, received number" }] },
                { "name": "b", "issues": [{ "message": "Required" }, { "message": "Required" }] }
            ]
        })]);
        assert_eq!(parsed.len(), 2);
        assert!(parsed.iter().all(|i| i.message == "Required"));
        assert!(parsed
            .iter()
            .all(|i| i.code.as_deref() == Some("invalid_union")));
    }

    #[test]
    fn empty_union_falls_back() {
        let parsed = parse_structured_errors(&[json!({
            "code": "invalid_union",
            "unionErrors": []
        })]);
        assert!(parsed[0].message.contains("expected format"));
    }

    #[test]
    fn missing_message_is_unknown() {
        let parsed = parse_structured_errors(&[json!({ "code": "custom" })]);
        assert_eq!(parsed[0].message, "Unknown error");
    }
}
