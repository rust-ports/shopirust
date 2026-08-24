//! Payments extension deploy config + Zod-equivalent validation.

use crate::error::AppError;
use serde_json::{json, Map, Value};

pub const OFFSITE_TARGET: &str = "payments.offsite.render";
pub const CREDIT_CARD_TARGET: &str = "payments.credit-card.render";
pub const CUSTOM_CREDIT_CARD_TARGET: &str = "payments.custom-credit-card.render";
pub const CUSTOM_ONSITE_TARGET: &str = "payments.custom-onsite.render";
pub const REDEEMABLE_TARGET: &str = "payments.redeemable.render";
pub const CARD_PRESENT_TARGET: &str = "payments.card-present.render";
pub const MAX_CHECKOUT_PAYMENT_METHOD_FIELDS: usize = 7;

pub fn payments_target(config: &Value) -> &str {
    config
        .pointer("/targeting/0/target")
        .and_then(|v| v.as_str())
        .unwrap_or("")
}

pub fn validate_payments(config: &Value) -> Result<(), AppError> {
    crate::models::extensions::schemas::require_string(config, "name")?;
    let target = payments_target(config);
    if target.is_empty() {
        return Err(AppError::message(
            "Payments extensions require a single targeting entry",
        ));
    }
    for required in ["api_version", "payment_session_url", "merchant_label"] {
        if config.get(required).and_then(|v| v.as_str()).is_none()
            && config.get(required).and_then(|v| v.as_bool()).is_none()
        {
            if required == "merchant_label" && config.get(required).is_some() {
                continue;
            }
            if config.get(required).is_none() {
                return Err(AppError::message(format!("{required} is required")));
            }
        }
    }

    let installments = config
        .get("supports_installments")
        .and_then(|v| v.as_bool());
    let deferred = config
        .get("supports_deferred_payments")
        .and_then(|v| v.as_bool());
    if matches!(
        target,
        OFFSITE_TARGET | CREDIT_CARD_TARGET | CUSTOM_CREDIT_CARD_TARGET | CUSTOM_ONSITE_TARGET
    ) && installments.is_some()
        && deferred.is_some()
        && installments != deferred
    {
        return Err(AppError::message(
            "supports_installments and supports_deferred_payments must be the same",
        ));
    }

    if target == OFFSITE_TARGET
        && config
            .get("supports_oversell_protection")
            .and_then(|v| v.as_bool())
            == Some(true)
        && config.get("confirmation_callback_url").is_none()
    {
        return Err(AppError::message(
            "Property required when supports_oversell_protection is true",
        ));
    }

    if target == REDEEMABLE_TARGET {
        let methods = config
            .get("supported_payment_methods")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();
        if methods.is_empty() {
            return Err(AppError::message(
                "supported_payment_methods is required for redeemable payments",
            ));
        }
    }

    if (target == CREDIT_CARD_TARGET || target == CUSTOM_CREDIT_CARD_TARGET)
        && config.get("encryption_certificate_fingerprint").is_none()
    {
        return Err(AppError::message(
            "encryption_certificate_fingerprint is required",
        ));
    }

    if matches!(
        target,
        CREDIT_CARD_TARGET | CUSTOM_CREDIT_CARD_TARGET | CUSTOM_ONSITE_TARGET
    ) && config.get("supports_3ds").and_then(|v| v.as_bool()) == Some(true)
        && config.get("confirmation_callback_url").is_none()
    {
        return Err(AppError::message(
            "Property required when supports_3ds is true",
        ));
    }

    if let Some(fields) = config
        .get("checkout_payment_method_fields")
        .and_then(|v| v.as_array())
    {
        if fields.len() > MAX_CHECKOUT_PAYMENT_METHOD_FIELDS {
            return Err(AppError::message(format!(
                "The extension can't have more than {MAX_CHECKOUT_PAYMENT_METHOD_FIELDS} checkout_payment_method_fields"
            )));
        }
    }

    if let Some(translations) = config
        .get("buyer_label_translations")
        .and_then(|v| v.as_array())
    {
        for t in translations {
            if t.get("locale").and_then(|v| v.as_str()).is_none() {
                return Err(AppError::message(
                    "buyer_label_translations locale is required",
                ));
            }
        }
    }

    Ok(())
}

pub fn deploy_payments(config: &Value) -> Result<Option<Value>, AppError> {
    let target = payments_target(config);

    let mut out = Map::new();
    out.insert(
        "api_version".into(),
        config.get("api_version").cloned().unwrap_or(Value::Null),
    );
    out.insert(
        "start_payment_session_url".into(),
        config
            .get("payment_session_url")
            .cloned()
            .unwrap_or(Value::Null),
    );
    for (from, to) in [
        ("refund_session_url", "start_refund_session_url"),
        ("capture_session_url", "start_capture_session_url"),
        ("void_session_url", "start_void_session_url"),
        ("verification_session_url", "start_verification_session_url"),
        ("update_payment_session_url", "update_payment_session_url"),
        ("confirmation_callback_url", "confirmation_callback_url"),
        ("balance_url", "balance_url"),
        (
            "sync_terminal_transaction_result_url",
            "sync_terminal_transaction_result_url",
        ),
    ] {
        if let Some(v) = config.get(from) {
            out.insert(to.into(), v.clone());
        }
    }
    for key in [
        "merchant_label",
        "supported_countries",
        "supported_payment_methods",
        "supported_buyer_contexts",
        "test_mode_available",
        "multiple_capture",
        "supports_3ds",
        "supports_deferred_payments",
        "supports_installments",
        "supports_oversell_protection",
        "supports_moto",
        "encryption_certificate_fingerprint",
        "checkout_payment_method_fields",
        "modal_payment_method_fields",
        "ui_extension_handle",
        "checkout_hosted_fields",
    ] {
        if let Some(v) = config.get(key) {
            out.insert(key.into(), v.clone());
        }
    }
    if let Some(label) = config.get("buyer_label") {
        out.insert("default_buyer_label".into(), label.clone());
    }
    if let Some(translations) = config.get("buyer_label_translations") {
        out.insert("buyer_label_to_locale".into(), translations.clone());
    }
    if target == REDEEMABLE_TARGET {
        let method = config
            .get("supported_payment_methods")
            .and_then(|v| v.as_array())
            .and_then(|a| a.first())
            .and_then(|v| v.as_str());
        out.insert(
            "redeemable_type".into(),
            if method == Some("gift-card") {
                json!("gift_card")
            } else {
                Value::Null
            },
        );
    }
    Ok(Some(Value::Object(out)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::extensions::deploy::{build_deploy_config, validate_configuration};
    use crate::models::extensions::specification::create_extension_specification;
    use std::collections::HashMap;
    use std::path::Path;

    fn offsite_cfg() -> HashMap<String, Value> {
        let mut c = HashMap::new();
        c.insert("name".into(), json!("test extension"));
        c.insert("type".into(), json!("payments_extension"));
        c.insert("payment_session_url".into(), json!("http://foo.bar"));
        c.insert("refund_session_url".into(), json!("http://foo.bar"));
        c.insert("capture_session_url".into(), json!("http://foo.bar"));
        c.insert("void_session_url".into(), json!("http://foo.bar"));
        c.insert("merchant_label".into(), json!("some-label"));
        c.insert("supported_countries".into(), json!(["CA"]));
        c.insert(
            "supported_payment_methods".into(),
            json!(["PAYMENT_METHOD"]),
        );
        c.insert("test_mode_available".into(), json!(true));
        c.insert("supports_3ds".into(), json!(false));
        c.insert("supports_deferred_payments".into(), json!(false));
        c.insert("supports_installments".into(), json!(false));
        c.insert("api_version".into(), json!("2022-07"));
        c.insert("targeting".into(), json!([{ "target": OFFSITE_TARGET }]));
        c
    }

    #[tokio::test]
    async fn offsite_deploy_config_maps_urls() {
        let spec = create_extension_specification("payments_extension").unwrap();
        let cfg = offsite_cfg();
        let out = build_deploy_config(&spec, &cfg, Path::new("."), &Default::default())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(out["start_payment_session_url"], "http://foo.bar");
        assert_eq!(out["start_refund_session_url"], "http://foo.bar");
        assert_eq!(out["merchant_label"], "some-label");
        assert_eq!(out["api_version"], "2022-07");
    }

    #[test]
    fn oversell_protection_requires_confirmation_url() {
        let spec = create_extension_specification("payments_extension").unwrap();
        let mut cfg = offsite_cfg();
        cfg.insert("supports_oversell_protection".into(), json!(true));
        let err = validate_configuration(&spec, &cfg, Path::new(".")).unwrap_err();
        assert!(err.to_string().contains("supports_oversell_protection"));
    }

    #[test]
    fn installments_must_match_deferred() {
        let spec = create_extension_specification("payments_extension").unwrap();
        let mut cfg = offsite_cfg();
        cfg.insert("supports_installments".into(), json!(true));
        cfg.insert("supports_deferred_payments".into(), json!(false));
        let err = validate_configuration(&spec, &cfg, Path::new(".")).unwrap_err();
        assert!(err.to_string().contains("must be the same"));
    }

    #[test]
    fn redeemable_maps_gift_card() {
        let mut cfg = offsite_cfg();
        cfg.insert("targeting".into(), json!([{ "target": REDEEMABLE_TARGET }]));
        cfg.insert("supported_payment_methods".into(), json!(["gift-card"]));
        let value = Value::Object(cfg.into_iter().collect());
        let out = deploy_payments(&value).unwrap().unwrap();
        assert_eq!(out["redeemable_type"], "gift_card");
    }

    #[test]
    fn credit_card_requires_fingerprint() {
        let spec = create_extension_specification("payments_extension").unwrap();
        let mut cfg = offsite_cfg();
        cfg.insert(
            "targeting".into(),
            json!([{ "target": CREDIT_CARD_TARGET }]),
        );
        let err = validate_configuration(&spec, &cfg, Path::new(".")).unwrap_err();
        assert!(err
            .to_string()
            .contains("encryption_certificate_fingerprint"));
    }

    #[test]
    fn missing_targeting_errors() {
        let spec = create_extension_specification("payments_extension").unwrap();
        let mut cfg = offsite_cfg();
        cfg.remove("targeting");
        let err = validate_configuration(&spec, &cfg, Path::new(".")).unwrap_err();
        assert!(err.to_string().contains("targeting"));
    }

    #[test]
    fn missing_payment_session_url_errors() {
        let spec = create_extension_specification("payments_extension").unwrap();
        let mut cfg = offsite_cfg();
        cfg.remove("payment_session_url");
        let err = validate_configuration(&spec, &cfg, Path::new(".")).unwrap_err();
        assert!(err.to_string().contains("payment_session_url"));
    }

    #[test]
    fn buyer_label_translations_require_locale() {
        let spec = create_extension_specification("payments_extension").unwrap();
        let mut cfg = offsite_cfg();
        cfg.insert(
            "buyer_label_translations".into(),
            json!([{ "label": "Translation without locale key" }]),
        );
        let err = validate_configuration(&spec, &cfg, Path::new(".")).unwrap_err();
        assert!(err.to_string().contains("locale"));
    }

    #[test]
    fn credit_card_3ds_requires_confirmation_url() {
        let spec = create_extension_specification("payments_extension").unwrap();
        let mut cfg = offsite_cfg();
        cfg.insert(
            "targeting".into(),
            json!([{ "target": CREDIT_CARD_TARGET }]),
        );
        cfg.insert(
            "encryption_certificate_fingerprint".into(),
            json!("fingerprint"),
        );
        cfg.insert("supports_3ds".into(), json!(true));
        let err = validate_configuration(&spec, &cfg, Path::new(".")).unwrap_err();
        assert!(err.to_string().contains("supports_3ds"));
    }

    #[test]
    fn checkout_payment_method_fields_cap() {
        let spec = create_extension_specification("payments_extension").unwrap();
        let mut cfg = offsite_cfg();
        cfg.insert(
            "targeting".into(),
            json!([{ "target": CREDIT_CARD_TARGET }]),
        );
        cfg.insert(
            "encryption_certificate_fingerprint".into(),
            json!("fingerprint"),
        );
        let fields: Vec<_> = (0..=MAX_CHECKOUT_PAYMENT_METHOD_FIELDS)
            .map(|i| json!({ "key": format!("key{i}"), "type": "string", "required": true }))
            .collect();
        cfg.insert("checkout_payment_method_fields".into(), json!(fields));
        let err = validate_configuration(&spec, &cfg, Path::new(".")).unwrap_err();
        assert!(err.to_string().contains("checkout_payment_method_fields"));
    }

    #[test]
    fn credit_card_valid_with_fingerprint() {
        let spec = create_extension_specification("payments_extension").unwrap();
        let mut cfg = offsite_cfg();
        cfg.insert(
            "targeting".into(),
            json!([{ "target": CREDIT_CARD_TARGET }]),
        );
        cfg.insert(
            "encryption_certificate_fingerprint".into(),
            json!("fingerprint"),
        );
        validate_configuration(&spec, &cfg, Path::new(".")).unwrap();
    }

    #[test]
    fn custom_onsite_validates() {
        let spec = create_extension_specification("payments_extension").unwrap();
        let mut cfg = offsite_cfg();
        cfg.insert(
            "targeting".into(),
            json!([{ "target": CUSTOM_ONSITE_TARGET }]),
        );
        cfg.insert("confirmation_callback_url".into(), json!("http://foo.bar"));
        cfg.insert("supports_3ds".into(), json!(true));
        cfg.insert("supports_installments".into(), json!(true));
        cfg.insert("supports_deferred_payments".into(), json!(true));
        validate_configuration(&spec, &cfg, Path::new(".")).unwrap();
    }

    #[test]
    fn card_present_validates() {
        let spec = create_extension_specification("payments_extension").unwrap();
        let mut cfg = offsite_cfg();
        cfg.insert(
            "targeting".into(),
            json!([{ "target": CARD_PRESENT_TARGET }]),
        );
        cfg.insert(
            "sync_terminal_transaction_result_url".into(),
            json!("http://foo.bar"),
        );
        validate_configuration(&spec, &cfg, Path::new(".")).unwrap();
    }

    #[test]
    fn redeemable_requires_methods() {
        let spec = create_extension_specification("payments_extension").unwrap();
        let mut cfg = offsite_cfg();
        cfg.insert("targeting".into(), json!([{ "target": REDEEMABLE_TARGET }]));
        cfg.insert("supported_payment_methods".into(), json!([]));
        let err = validate_configuration(&spec, &cfg, Path::new(".")).unwrap_err();
        assert!(err.to_string().contains("supported_payment_methods"));
    }

    #[tokio::test]
    async fn credit_card_deploy_maps_fingerprint_and_moto() {
        let spec = create_extension_specification("payments_extension").unwrap();
        let mut cfg = offsite_cfg();
        cfg.insert(
            "targeting".into(),
            json!([{ "target": CREDIT_CARD_TARGET }]),
        );
        cfg.insert(
            "encryption_certificate_fingerprint".into(),
            json!("fingerprint"),
        );
        cfg.insert("supports_moto".into(), json!(true));
        cfg.insert("buyer_label".into(), json!("Pay now"));
        let out = build_deploy_config(&spec, &cfg, Path::new("."), &Default::default())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(out["encryption_certificate_fingerprint"], "fingerprint");
        assert_eq!(out["supports_moto"], true);
        assert_eq!(out["default_buyer_label"], "Pay now");
    }

    #[tokio::test]
    async fn card_present_deploy_maps_sync_url() {
        let spec = create_extension_specification("payments_extension").unwrap();
        let mut cfg = offsite_cfg();
        cfg.insert(
            "targeting".into(),
            json!([{ "target": CARD_PRESENT_TARGET }]),
        );
        cfg.insert(
            "sync_terminal_transaction_result_url".into(),
            json!("http://sync"),
        );
        let out = build_deploy_config(&spec, &cfg, Path::new("."), &Default::default())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(out["sync_terminal_transaction_result_url"], "http://sync");
    }
}
