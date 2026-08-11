//! Payments extension import helpers (upstream `services/payments/`).

use crate::error::AppError;
use crate::models::extensions::schemas::MAX_EXTENSION_HANDLE_LENGTH;
use crate::services::generate::slugify;
use crate::services::import_extensions::ExtensionRegistration;
use serde_json::{json, Map, Value};

pub const OFFSITE_TARGET: &str = "payments.offsite.render";
pub const CREDIT_CARD_TARGET: &str = "payments.credit-card.render";
pub const CUSTOM_CREDIT_CARD_TARGET: &str = "payments.custom-credit-card.render";
pub const CUSTOM_ONSITE_TARGET: &str = "payments.custom-onsite.render";
pub const REDEEMABLE_TARGET: &str = "payments.redeemable.render";
pub const CARD_PRESENT_TARGET: &str = "payments.card-present.render";

/// Dashboard payment registration type → target context.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DashboardPaymentExtensionType {
    Offsite,
    CreditCard,
    CustomCreditCard,
    CustomOnsite,
    Redeemable,
    CardPresent,
}

impl DashboardPaymentExtensionType {
    pub fn as_type_name(self) -> &'static str {
        match self {
            Self::Offsite => "payments_app",
            Self::CreditCard => "payments_app_credit_card",
            Self::CustomCreditCard => "payments_app_custom_credit_card",
            Self::CustomOnsite => "payments_app_custom_onsite",
            Self::Redeemable => "payments_app_redeemable",
            Self::CardPresent => "payments_app_card_present",
        }
    }

    pub fn from_type_name(s: &str) -> Option<Self> {
        match s {
            "payments_app" => Some(Self::Offsite),
            "payments_app_credit_card" => Some(Self::CreditCard),
            "payments_app_custom_credit_card" => Some(Self::CustomCreditCard),
            "payments_app_custom_onsite" => Some(Self::CustomOnsite),
            "payments_app_redeemable" => Some(Self::Redeemable),
            "payments_app_card_present" => Some(Self::CardPresent),
            _ => None,
        }
    }

    pub fn target(self) -> &'static str {
        match self {
            Self::Offsite => OFFSITE_TARGET,
            Self::CreditCard => CREDIT_CARD_TARGET,
            Self::CustomCreditCard => CUSTOM_CREDIT_CARD_TARGET,
            Self::CustomOnsite => CUSTOM_ONSITE_TARGET,
            Self::Redeemable => REDEEMABLE_TARGET,
            Self::CardPresent => CARD_PRESENT_TARGET,
        }
    }
}

fn truncated_handle(title: &str) -> String {
    let truncated: String = title.chars().take(MAX_EXTENSION_HANDLE_LENGTH).collect();
    slugify(&truncated)
}

fn type_to_context(type_name: &str) -> Option<&'static str> {
    DashboardPaymentExtensionType::from_type_name(type_name).map(|t| t.target())
}

fn version_of(ext: &ExtensionRegistration) -> Result<&crate::services::import_extensions::ExtensionVersion, AppError> {
    ext.active_version
        .as_ref()
        .or(ext.draft_version.as_ref())
        .ok_or_else(|| AppError::message("No config found for extension"))
}

fn extension_uuid_to_handle(config: &Value, all_extensions: &[ExtensionRegistration]) -> Option<String> {
    if let Some(handle) = config.get("ui_extension_handle").and_then(|v| v.as_str()) {
        return Some(handle.to_string());
    }
    let uuid = config
        .get("ui_extension_registration_uuid")
        .and_then(|v| v.as_str())?;
    all_extensions
        .iter()
        .find(|e| e.uuid == uuid)
        .map(|e| slugify(&e.title))
}

fn take_str(config: &Value, key: &str) -> Option<Value> {
    config.get(key).cloned()
}

fn map_common_session_urls(config: &Value, out: &mut Map<String, Value>) {
    if let Some(v) = take_str(config, "start_payment_session_url") {
        out.insert("payment_session_url".into(), v);
    }
    for (from, to) in [
        ("start_refund_session_url", "refund_session_url"),
        ("start_capture_session_url", "capture_session_url"),
        ("start_void_session_url", "void_session_url"),
    ] {
        if let Some(v) = take_str(config, from) {
            out.insert(to.into(), v);
        }
    }
}

fn map_buyer_labels(config: &Value, out: &mut Map<String, Value>) {
    if let Some(v) = take_str(config, "default_buyer_label") {
        out.insert("buyer_label".into(), v);
    }
    if let Some(v) = take_str(config, "buyer_label_to_locale") {
        out.insert("buyer_label_translations".into(), v);
    }
}

fn copy_keys(config: &Value, out: &mut Map<String, Value>, keys: &[&str]) {
    for key in keys {
        if let Some(v) = config.get(*key) {
            out.insert((*key).into(), v.clone());
        }
    }
}

/// Deploy JSON → CLI TOML fields for offsite payments.
pub fn offsite_deploy_config_to_cli(config: &Value) -> Map<String, Value> {
    let mut out = Map::new();
    copy_keys(config, &mut out, &["api_version"]);
    map_common_session_urls(config, &mut out);
    copy_keys(
        config,
        &mut out,
        &[
            "confirmation_callback_url",
            "multiple_capture",
            "merchant_label",
            "supported_countries",
            "supported_payment_methods",
            "supported_buyer_contexts",
            "test_mode_available",
            "supports_oversell_protection",
            "supports_3ds",
            "supports_deferred_payments",
            "supports_installments",
        ],
    );
    map_buyer_labels(config, &mut out);
    out
}

/// Deploy JSON → CLI TOML fields for credit-card payments.
pub fn credit_card_deploy_config_to_cli(
    config: &Value,
    all_extensions: &[ExtensionRegistration],
) -> Map<String, Value> {
    let mut out = Map::new();
    copy_keys(config, &mut out, &["api_version"]);
    map_common_session_urls(config, &mut out);
    copy_keys(
        config,
        &mut out,
        &[
            "confirmation_callback_url",
            "multiple_capture",
            "merchant_label",
            "supported_countries",
            "supported_payment_methods",
            "supported_buyer_contexts",
            "test_mode_available",
            "supports_3ds",
            "supports_moto",
            "supports_deferred_payments",
            "supports_installments",
            "checkout_payment_method_fields",
        ],
    );
    if let Some(v) = take_str(config, "start_verification_session_url") {
        out.insert("verification_session_url".into(), v);
    }
    if let Some(fp) = config
        .pointer("/encryption_certificate/fingerprint")
        .cloned()
    {
        out.insert("encryption_certificate_fingerprint".into(), fp);
    }
    if let Some(handle) = extension_uuid_to_handle(config, all_extensions) {
        out.insert("ui_extension_handle".into(), Value::String(handle));
    }
    out
}

pub fn custom_credit_card_deploy_config_to_cli(
    config: &Value,
    all_extensions: &[ExtensionRegistration],
) -> Map<String, Value> {
    let mut out = Map::new();
    copy_keys(config, &mut out, &["api_version"]);
    map_common_session_urls(config, &mut out);
    copy_keys(
        config,
        &mut out,
        &[
            "confirmation_callback_url",
            "merchant_label",
            "supports_3ds",
            "supported_countries",
            "supported_payment_methods",
            "supported_buyer_contexts",
            "test_mode_available",
            "multiple_capture",
            "checkout_payment_method_fields",
            "checkout_hosted_fields",
        ],
    );
    map_buyer_labels(config, &mut out);
    if let Some(fp) = config
        .pointer("/encryption_certificate/fingerprint")
        .cloned()
    {
        out.insert("encryption_certificate_fingerprint".into(), fp);
    }
    if let Some(handle) = extension_uuid_to_handle(config, all_extensions) {
        out.insert("ui_extension_handle".into(), Value::String(handle));
    }
    out
}

pub fn custom_onsite_deploy_config_to_cli(
    config: &Value,
    all_extensions: &[ExtensionRegistration],
) -> Map<String, Value> {
    let mut out = Map::new();
    copy_keys(config, &mut out, &["api_version"]);
    map_common_session_urls(config, &mut out);
    copy_keys(
        config,
        &mut out,
        &[
            "confirmation_callback_url",
            "update_payment_session_url",
            "start_verification_session_url",
            "merchant_label",
            "supports_oversell_protection",
            "supports_3ds",
            "supports_installments",
            "supports_deferred_payments",
            "supported_countries",
            "supported_payment_methods",
            "supported_buyer_contexts",
            "test_mode_available",
            "multiple_capture",
            "checkout_payment_method_fields",
            "modal_payment_method_fields",
        ],
    );
    map_buyer_labels(config, &mut out);
    if let Some(handle) = extension_uuid_to_handle(config, all_extensions) {
        out.insert("ui_extension_handle".into(), Value::String(handle));
    }
    out
}

pub fn redeemable_deploy_config_to_cli(
    config: &Value,
    all_extensions: &[ExtensionRegistration],
) -> Map<String, Value> {
    let mut out = Map::new();
    copy_keys(config, &mut out, &["api_version"]);
    map_common_session_urls(config, &mut out);
    copy_keys(
        config,
        &mut out,
        &[
            "merchant_label",
            "supported_countries",
            "supported_payment_methods",
            "supported_buyer_contexts",
            "test_mode_available",
            "balance_url",
            "checkout_payment_method_fields",
        ],
    );
    map_buyer_labels(config, &mut out);
    if let Some(handle) = extension_uuid_to_handle(config, all_extensions) {
        out.insert("ui_extension_handle".into(), Value::String(handle));
    }
    out
}

pub fn card_present_deploy_config_to_cli(config: &Value) -> Map<String, Value> {
    let mut out = Map::new();
    copy_keys(config, &mut out, &["api_version"]);
    map_common_session_urls(config, &mut out);
    copy_keys(
        config,
        &mut out,
        &[
            "sync_terminal_transaction_result_url",
            "merchant_label",
            "supported_countries",
            "supported_payment_methods",
            "test_mode_available",
        ],
    );
    out
}

fn deploy_config_to_cli(
    target: &str,
    config: &Value,
    all_extensions: &[ExtensionRegistration],
) -> Result<Map<String, Value>, AppError> {
    let mut cli = match target {
        OFFSITE_TARGET => offsite_deploy_config_to_cli(config),
        CREDIT_CARD_TARGET => credit_card_deploy_config_to_cli(config, all_extensions),
        CUSTOM_CREDIT_CARD_TARGET => {
            custom_credit_card_deploy_config_to_cli(config, all_extensions)
        }
        CUSTOM_ONSITE_TARGET => custom_onsite_deploy_config_to_cli(config, all_extensions),
        REDEEMABLE_TARGET => redeemable_deploy_config_to_cli(config, all_extensions),
        CARD_PRESENT_TARGET => card_present_deploy_config_to_cli(config),
        other => {
            return Err(AppError::message(format!(
                "Unsupported extension: {other}"
            )))
        }
    };
    cli.remove("api_version");
    Ok(cli)
}

/// Convert a dashboard payments registration into local unified TOML JSON.
pub fn build_extension_config(
    extension: &ExtensionRegistration,
    all_extensions: &[ExtensionRegistration],
) -> Result<Value, AppError> {
    let version = version_of(extension)?;
    let version_config = version
        .config
        .as_deref()
        .ok_or_else(|| AppError::message("No config found for extension"))?;
    let dashboard_config: Value = serde_json::from_str(version_config)?;

    let context = version
        .context
        .as_deref()
        .or_else(|| type_to_context(&extension.type_name))
        .ok_or_else(|| AppError::message("Unsupported extension: "))?;

    let mut cli_config = deploy_config_to_cli(context, &dashboard_config, all_extensions)?;

    let mut extension_obj = Map::new();
    extension_obj.insert("name".into(), Value::String(extension.title.clone()));
    extension_obj.insert("type".into(), Value::String("payments_extension".into()));
    extension_obj.insert(
        "handle".into(),
        Value::String(truncated_handle(&extension.title)),
    );
    extension_obj.append(&mut cli_config);
    extension_obj.insert(
        "targeting".into(),
        json!([{ "target": context }]),
    );

    Ok(json!({
        "api_version": dashboard_config.get("api_version").cloned().unwrap_or(Value::Null),
        "extensions": [Value::Object(extension_obj)],
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::import_extensions::ExtensionVersion;

    const SAMPLE_OFFSITE: &str = r#"{"start_payment_session_url":"https://bogus-app/payment-sessions/start","start_refund_session_url":"https://bogus-app/payment-sessions/refund","start_capture_session_url":"https://bogus-app/payment-sessions/capture","start_void_session_url":"https://bogus-app/payment-sessions/void","confirmation_callback_url":"https://bogus-app/payment-sessions/confirm","supported_payment_methods":["visa","master"],"supported_countries":["GG"],"test_mode_available":true,"merchant_label":"Offsite Payments App Extension","default_buyer_label":null,"buyer_label_to_locale":null,"supports_3ds":true,"supports_oversell_protection":false,"api_version":"2023-10","supports_installments":true,"supports_deferred_payments":true,"multiple_capture":false,"supported_buyer_contexts":[{"currency":"USD","countries":["US"]}]}"#;

    #[test]
    fn builds_offsite_payments_config() {
        let extension = ExtensionRegistration {
            uuid: "626ab61a-e494-4e16-b511-e8721ec011a4".into(),
            title: "Bogus Pay".into(),
            type_name: "payments_extension".into(),
            draft_version: Some(ExtensionVersion {
                config: Some(SAMPLE_OFFSITE.into()),
                context: Some(OFFSITE_TARGET.into()),
            }),
            active_version: None,
        };
        let got = build_extension_config(&extension, &[extension.clone()]).unwrap();
        assert_eq!(got.get("api_version").and_then(|v| v.as_str()), Some("2023-10"));
        assert_eq!(
            got.pointer("/extensions/0/payment_session_url")
                .and_then(|v| v.as_str()),
            Some("https://bogus-app/payment-sessions/start")
        );
        assert_eq!(
            got.pointer("/extensions/0/targeting/0/target")
                .and_then(|v| v.as_str()),
            Some(OFFSITE_TARGET)
        );
        assert_eq!(
            got.pointer("/extensions/0/handle").and_then(|v| v.as_str()),
            Some("bogus-pay")
        );
    }

    #[test]
    fn truncates_payments_handle() {
        let extension = ExtensionRegistration {
            uuid: "u".into(),
            title: "Bogus Pay Bogus Pay Bogus Pay Bogus Pay Bogus Pay Bogus Pay Bogus".into(),
            type_name: "payments_app".into(),
            draft_version: Some(ExtensionVersion {
                config: Some(SAMPLE_OFFSITE.into()),
                context: Some(OFFSITE_TARGET.into()),
            }),
            active_version: None,
        };
        let got = build_extension_config(&extension, std::slice::from_ref(&extension)).unwrap();
        assert_eq!(
            got.pointer("/extensions/0/handle").and_then(|v| v.as_str()),
            Some("bogus-pay-bogus-pay-bogus-pay-bogus-pay-bogus-pay")
        );
    }

    #[test]
    fn maps_credit_card_ui_extension_handle() {
        let ui = ExtensionRegistration {
            uuid: "3f9d1c40-0f7d-48f9-b802-ca7d302ee8bc".into(),
            title: "Checkout UI".into(),
            type_name: "ui_extension".into(),
            draft_version: None,
            active_version: None,
        };
        let config = json!({
            "api_version": "2023-04",
            "start_payment_session_url": "https://test-domain.com/authorize",
            "start_refund_session_url": "https://test-domain.com/refund",
            "start_capture_session_url": "https://test-domain.com/capture",
            "start_void_session_url": "https://test-domain.com/void",
            "merchant_label": "test-label",
            "supported_countries": ["JP"],
            "supported_payment_methods": ["visa"],
            "test_mode_available": true,
            "supports_3ds": true,
            "supports_moto": false,
            "supports_installments": false,
            "supports_deferred_payments": false,
            "encryption_certificate": {"fingerprint": "fingerprint", "certificate": "certificate"},
            "ui_extension_registration_uuid": "3f9d1c40-0f7d-48f9-b802-ca7d302ee8bc"
        });
        let cli = credit_card_deploy_config_to_cli(&config, &[ui]);
        assert_eq!(
            cli.get("ui_extension_handle").and_then(|v| v.as_str()),
            Some("checkout-ui")
        );
        assert_eq!(
            cli.get("encryption_certificate_fingerprint")
                .and_then(|v| v.as_str()),
            Some("fingerprint")
        );
    }

    #[test]
    fn card_present_mapping() {
        let config = json!({
            "api_version": "2025-04",
            "start_payment_session_url": "https://x/pay",
            "start_refund_session_url": "https://x/refund",
            "start_capture_session_url": "https://x/capture",
            "start_void_session_url": "https://x/void",
            "sync_terminal_transaction_result_url": "https://x/terminal",
            "merchant_label": "Card Present",
            "supported_countries": ["US"],
            "supported_payment_methods": ["visa"],
            "test_mode_available": true
        });
        let cli = card_present_deploy_config_to_cli(&config);
        assert_eq!(
            cli.get("sync_terminal_transaction_result_url")
                .and_then(|v| v.as_str()),
            Some("https://x/terminal")
        );
        assert_eq!(
            cli.get("payment_session_url").and_then(|v| v.as_str()),
            Some("https://x/pay")
        );
    }
}
