use crate::error::AppError;

const MAX_LEN: usize = 200;

/// Validate an optional deploy/release message.
pub fn validate_message(message: Option<&str>) -> Result<(), AppError> {
    let Some(message) = message else {
        return Ok(());
    };
    if message.len() > MAX_LEN {
        return Err(AppError::message(format!(
            "Invalid message: {message}\nMessage name must be 200 characters or less."
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_short() {
        validate_message(Some("hello")).unwrap();
        validate_message(None).unwrap();
    }

    #[test]
    fn rejects_too_long() {
        let s = "A".repeat(201);
        assert!(validate_message(Some(&s)).is_err());
    }
}
