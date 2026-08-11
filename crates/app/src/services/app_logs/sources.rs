use crate::models::loader::LoadedApp;

/// Function extension sources as `extensions.<handle>` (upstream `sourcesForApp`).
pub fn sources_for_app(app: &LoadedApp) -> Vec<String> {
    app.all_extensions()
        .iter()
        .filter(|ext| ext.is_function_extension())
        .map(|ext| format!("extensions.{}", ext.handle))
        .collect()
}

/// Group sources by namespace and format for CLI output.
pub fn format_sources_output(app: &LoadedApp) -> String {
    let sources = sources_for_app(app);
    let mut by_namespace: Vec<(String, Vec<String>)> = Vec::new();

    for source in sources {
        let mut tokens = source.splitn(2, '.');
        let Some(namespace) = tokens.next() else {
            continue;
        };
        if tokens.next().is_none() {
            continue;
        }
        if let Some((_, list)) = by_namespace.iter_mut().find(|(ns, _)| ns == namespace) {
            list.push(source);
        } else {
            by_namespace.push((namespace.to_string(), vec![source]));
        }
    }

    let mut out = String::new();
    for (namespace, list) in by_namespace {
        out.push_str(&format!("╭─ {namespace} ───────────────────────────────────────\n"));
        for s in list {
            out.push_str(&s);
            out.push('\n');
        }
        out.push_str("╰────────────────────────────────────────────────────\n");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::app::AppHiddenConfig;
    use crate::models::extensions::extension_instance::ExtensionInstance;
    use crate::models::extensions::specifications::function_specification;
    use crate::models::identifiers::Identifiers;
    use crate::models::AppConfiguration;
    use std::collections::HashMap;
    use std::path::PathBuf;

    fn function_ext(handle: &str) -> ExtensionInstance {
        ExtensionInstance::new(
            handle,
            PathBuf::from("extensions").join(handle),
            PathBuf::from("shopify.extension.toml"),
            HashMap::new(),
            function_specification(),
        )
    }

    #[test]
    fn sources_for_function_extensions() {
        let app = LoadedApp {
            name: "Demo".into(),
            directory: PathBuf::from("."),
            configuration_path: PathBuf::from("shopify.app.toml"),
            configuration: AppConfiguration::default(),
            hidden_config: AppHiddenConfig::default(),
            extensions: vec![function_ext("discount"), function_ext("delivery")],
            webs: vec![],
            identifiers: Identifiers::default(),
            errors: vec![],
        };
        let sources = sources_for_app(&app);
        assert_eq!(
            sources,
            vec![
                "extensions.discount".to_string(),
                "extensions.delivery".to_string()
            ]
        );
    }
}
