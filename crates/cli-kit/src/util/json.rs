use crate::error::{FatalError, FatalErrorType};

pub fn parse_json<T: serde::de::DeserializeOwned>(
    json_string: &str,
    context: Option<&str>,
) -> Result<T, FatalError> {
    serde_json::from_str::<T>(json_string).map_err(|e| {
        let ctx = context.unwrap_or("JSON");
        FatalError {
            message: format!("Failed to parse {}: {}", ctx, e),
            r#type: FatalErrorType::Abort,
            try_message: Some("Check the file format and try again".into()),
            next_steps: vec![],
            custom_sections: vec![],
            formatted_message: None,
            skip_oclif_error_handling: true,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;

    #[derive(Deserialize, PartialEq, Debug)]
    struct TestData {
        name: String,
        value: i32,
    }

    #[test]
    fn test_parse_json_valid() {
        let data: TestData = parse_json(r#"{"name": "test", "value": 42}"#, None).unwrap();
        assert_eq!(
            data,
            TestData {
                name: "test".into(),
                value: 42
            }
        );
    }

    #[test]
    fn test_parse_json_invalid() {
        let result: Result<TestData, FatalError> = parse_json("invalid json", None);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().r#type, FatalErrorType::Abort);
    }

    #[test]
    fn test_parse_json_with_context() {
        let result: Result<serde_json::Value, FatalError> = parse_json("bad", Some("config file"));
        assert!(result.is_err());
        assert!(result.unwrap_err().message.contains("config file"));
    }
}
