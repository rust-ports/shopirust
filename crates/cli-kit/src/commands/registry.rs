#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DispatchMode {
    Native,
    Bridge,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CommandInfo {
    pub id: &'static str,
    pub hidden: bool,
    pub dispatch: DispatchMode,
}

pub const COMMANDS: &[CommandInfo] = &[
    CommandInfo {
        id: "app:build",
        hidden: false,
        dispatch: DispatchMode::Native,
    },
    CommandInfo {
        id: "app:bulk:cancel",
        hidden: false,
        dispatch: DispatchMode::Native,
    },
    CommandInfo {
        id: "app:bulk:execute",
        hidden: false,
        dispatch: DispatchMode::Native,
    },
    CommandInfo {
        id: "app:bulk:status",
        hidden: false,
        dispatch: DispatchMode::Native,
    },
    CommandInfo {
        id: "app:config:link",
        hidden: false,
        dispatch: DispatchMode::Native,
    },
    CommandInfo {
        id: "app:config:pull",
        hidden: false,
        dispatch: DispatchMode::Native,
    },
    CommandInfo {
        id: "app:config:use",
        hidden: false,
        dispatch: DispatchMode::Native,
    },
    CommandInfo {
        id: "app:config:validate",
        hidden: false,
        dispatch: DispatchMode::Native,
    },
    CommandInfo {
        id: "app:deploy",
        hidden: false,
        dispatch: DispatchMode::Native,
    },
    CommandInfo {
        id: "app:dev",
        hidden: false,
        dispatch: DispatchMode::Native,
    },
    CommandInfo {
        id: "app:dev:clean",
        hidden: false,
        dispatch: DispatchMode::Native,
    },
    CommandInfo {
        id: "app:env:pull",
        hidden: false,
        dispatch: DispatchMode::Native,
    },
    CommandInfo {
        id: "app:env:show",
        hidden: false,
        dispatch: DispatchMode::Native,
    },
    CommandInfo {
        id: "app:execute",
        hidden: false,
        dispatch: DispatchMode::Native,
    },
    CommandInfo {
        id: "app:function:build",
        hidden: false,
        dispatch: DispatchMode::Native,
    },
    CommandInfo {
        id: "app:function:info",
        hidden: false,
        dispatch: DispatchMode::Native,
    },
    CommandInfo {
        id: "app:function:replay",
        hidden: false,
        dispatch: DispatchMode::Native,
    },
    CommandInfo {
        id: "app:function:run",
        hidden: false,
        dispatch: DispatchMode::Native,
    },
    CommandInfo {
        id: "app:function:schema",
        hidden: false,
        dispatch: DispatchMode::Native,
    },
    CommandInfo {
        id: "app:function:typegen",
        hidden: false,
        dispatch: DispatchMode::Native,
    },
    CommandInfo {
        id: "app:generate:extension",
        hidden: false,
        dispatch: DispatchMode::Native,
    },
    CommandInfo {
        id: "app:import-custom-data-definitions",
        hidden: false,
        dispatch: DispatchMode::Native,
    },
    CommandInfo {
        id: "app:import-extensions",
        hidden: false,
        dispatch: DispatchMode::Native,
    },
    CommandInfo {
        id: "app:info",
        hidden: false,
        dispatch: DispatchMode::Native,
    },
    CommandInfo {
        id: "app:init",
        hidden: false,
        dispatch: DispatchMode::Native,
    },
    CommandInfo {
        id: "app:logs",
        hidden: false,
        dispatch: DispatchMode::Native,
    },
    CommandInfo {
        id: "app:logs:sources",
        hidden: false,
        dispatch: DispatchMode::Native,
    },
    CommandInfo {
        id: "app:release",
        hidden: false,
        dispatch: DispatchMode::Native,
    },
    CommandInfo {
        id: "app:versions:list",
        hidden: false,
        dispatch: DispatchMode::Native,
    },
    CommandInfo {
        id: "app:webhook:trigger",
        hidden: false,
        dispatch: DispatchMode::Native,
    },
    CommandInfo {
        id: "auth:login",
        hidden: false,
        dispatch: DispatchMode::Native,
    },
    CommandInfo {
        id: "auth:logout",
        hidden: false,
        dispatch: DispatchMode::Native,
    },
    CommandInfo {
        id: "cache:clear",
        hidden: true,
        dispatch: DispatchMode::Native,
    },
    CommandInfo {
        id: "commands",
        hidden: false,
        dispatch: DispatchMode::Native,
    },
    CommandInfo {
        id: "config:autocorrect:off",
        hidden: false,
        dispatch: DispatchMode::Native,
    },
    CommandInfo {
        id: "config:autocorrect:on",
        hidden: false,
        dispatch: DispatchMode::Native,
    },
    CommandInfo {
        id: "config:autocorrect:status",
        hidden: false,
        dispatch: DispatchMode::Native,
    },
    CommandInfo {
        id: "config:autoupgrade:off",
        hidden: false,
        dispatch: DispatchMode::Native,
    },
    CommandInfo {
        id: "config:autoupgrade:on",
        hidden: false,
        dispatch: DispatchMode::Native,
    },
    CommandInfo {
        id: "config:autoupgrade:status",
        hidden: false,
        dispatch: DispatchMode::Native,
    },
    CommandInfo {
        id: "debug:command-flags",
        hidden: true,
        dispatch: DispatchMode::Native,
    },
    CommandInfo {
        id: "demo:watcher",
        hidden: true,
        dispatch: DispatchMode::Native,
    },
    CommandInfo {
        id: "docs:generate",
        hidden: true,
        dispatch: DispatchMode::Native,
    },
    CommandInfo {
        id: "doctor-release",
        hidden: true,
        dispatch: DispatchMode::Bridge,
    },
    CommandInfo {
        id: "doctor-release:theme",
        hidden: true,
        dispatch: DispatchMode::Bridge,
    },
    CommandInfo {
        id: "help",
        hidden: false,
        dispatch: DispatchMode::Native,
    },
    CommandInfo {
        id: "hydrogen:build",
        hidden: false,
        dispatch: DispatchMode::Bridge,
    },
    CommandInfo {
        id: "hydrogen:check",
        hidden: false,
        dispatch: DispatchMode::Bridge,
    },
    CommandInfo {
        id: "hydrogen:codegen",
        hidden: false,
        dispatch: DispatchMode::Bridge,
    },
    CommandInfo {
        id: "hydrogen:customer-account-push",
        hidden: false,
        dispatch: DispatchMode::Bridge,
    },
    CommandInfo {
        id: "hydrogen:debug:cpu",
        hidden: false,
        dispatch: DispatchMode::Bridge,
    },
    CommandInfo {
        id: "hydrogen:deploy",
        hidden: false,
        dispatch: DispatchMode::Bridge,
    },
    CommandInfo {
        id: "hydrogen:dev",
        hidden: false,
        dispatch: DispatchMode::Bridge,
    },
    CommandInfo {
        id: "hydrogen:env:list",
        hidden: false,
        dispatch: DispatchMode::Bridge,
    },
    CommandInfo {
        id: "hydrogen:env:pull",
        hidden: false,
        dispatch: DispatchMode::Bridge,
    },
    CommandInfo {
        id: "hydrogen:env:push",
        hidden: false,
        dispatch: DispatchMode::Bridge,
    },
    CommandInfo {
        id: "hydrogen:g",
        hidden: true,
        dispatch: DispatchMode::Bridge,
    },
    CommandInfo {
        id: "hydrogen:generate:route",
        hidden: false,
        dispatch: DispatchMode::Bridge,
    },
    CommandInfo {
        id: "hydrogen:generate:routes",
        hidden: false,
        dispatch: DispatchMode::Bridge,
    },
    CommandInfo {
        id: "hydrogen:init",
        hidden: false,
        dispatch: DispatchMode::Bridge,
    },
    CommandInfo {
        id: "hydrogen:link",
        hidden: false,
        dispatch: DispatchMode::Bridge,
    },
    CommandInfo {
        id: "hydrogen:list",
        hidden: false,
        dispatch: DispatchMode::Bridge,
    },
    CommandInfo {
        id: "hydrogen:login",
        hidden: false,
        dispatch: DispatchMode::Bridge,
    },
    CommandInfo {
        id: "hydrogen:logout",
        hidden: false,
        dispatch: DispatchMode::Bridge,
    },
    CommandInfo {
        id: "hydrogen:preview",
        hidden: false,
        dispatch: DispatchMode::Bridge,
    },
    CommandInfo {
        id: "hydrogen:setup",
        hidden: false,
        dispatch: DispatchMode::Bridge,
    },
    CommandInfo {
        id: "hydrogen:setup:css",
        hidden: false,
        dispatch: DispatchMode::Bridge,
    },
    CommandInfo {
        id: "hydrogen:setup:markets",
        hidden: false,
        dispatch: DispatchMode::Bridge,
    },
    CommandInfo {
        id: "hydrogen:setup:vite",
        hidden: false,
        dispatch: DispatchMode::Bridge,
    },
    CommandInfo {
        id: "hydrogen:shortcut",
        hidden: false,
        dispatch: DispatchMode::Bridge,
    },
    CommandInfo {
        id: "hydrogen:unlink",
        hidden: false,
        dispatch: DispatchMode::Bridge,
    },
    CommandInfo {
        id: "hydrogen:upgrade",
        hidden: false,
        dispatch: DispatchMode::Bridge,
    },
    CommandInfo {
        id: "kitchen-sink",
        hidden: true,
        dispatch: DispatchMode::Native,
    },
    CommandInfo {
        id: "kitchen-sink:async",
        hidden: true,
        dispatch: DispatchMode::Native,
    },
    CommandInfo {
        id: "kitchen-sink:prompts",
        hidden: true,
        dispatch: DispatchMode::Native,
    },
    CommandInfo {
        id: "kitchen-sink:static",
        hidden: true,
        dispatch: DispatchMode::Native,
    },
    CommandInfo {
        id: "notifications:generate",
        hidden: true,
        dispatch: DispatchMode::Native,
    },
    CommandInfo {
        id: "notifications:list",
        hidden: true,
        dispatch: DispatchMode::Native,
    },
    CommandInfo {
        id: "organization:list",
        hidden: false,
        dispatch: DispatchMode::Native,
    },
    CommandInfo {
        id: "plugins",
        hidden: true,
        dispatch: DispatchMode::Bridge,
    },
    CommandInfo {
        id: "plugins:inspect",
        hidden: false,
        dispatch: DispatchMode::Bridge,
    },
    CommandInfo {
        id: "plugins:install",
        hidden: false,
        dispatch: DispatchMode::Bridge,
    },
    CommandInfo {
        id: "plugins:link",
        hidden: false,
        dispatch: DispatchMode::Bridge,
    },
    CommandInfo {
        id: "plugins:reset",
        hidden: false,
        dispatch: DispatchMode::Bridge,
    },
    CommandInfo {
        id: "plugins:uninstall",
        hidden: false,
        dispatch: DispatchMode::Bridge,
    },
    CommandInfo {
        id: "plugins:update",
        hidden: false,
        dispatch: DispatchMode::Bridge,
    },
    CommandInfo {
        id: "search",
        hidden: false,
        dispatch: DispatchMode::Native,
    },
    CommandInfo {
        id: "store:auth",
        hidden: false,
        dispatch: DispatchMode::Native,
    },
    CommandInfo {
        id: "store:auth:list",
        hidden: true,
        dispatch: DispatchMode::Native,
    },
    CommandInfo {
        id: "store:create:dev",
        hidden: true,
        dispatch: DispatchMode::Native,
    },
    CommandInfo {
        id: "store:create:preview",
        hidden: true,
        dispatch: DispatchMode::Native,
    },
    CommandInfo {
        id: "store:execute",
        hidden: false,
        dispatch: DispatchMode::Native,
    },
    CommandInfo {
        id: "store:info",
        hidden: false,
        dispatch: DispatchMode::Native,
    },
    CommandInfo {
        id: "store:list",
        hidden: true,
        dispatch: DispatchMode::Native,
    },
    CommandInfo {
        id: "theme:check",
        hidden: false,
        dispatch: DispatchMode::Native,
    },
    CommandInfo {
        id: "theme:console",
        hidden: false,
        dispatch: DispatchMode::Native,
    },
    CommandInfo {
        id: "theme:delete",
        hidden: false,
        dispatch: DispatchMode::Native,
    },
    CommandInfo {
        id: "theme:dev",
        hidden: false,
        dispatch: DispatchMode::Native,
    },
    CommandInfo {
        id: "theme:duplicate",
        hidden: false,
        dispatch: DispatchMode::Native,
    },
    CommandInfo {
        id: "theme:info",
        hidden: false,
        dispatch: DispatchMode::Native,
    },
    CommandInfo {
        id: "theme:init",
        hidden: false,
        dispatch: DispatchMode::Native,
    },
    CommandInfo {
        id: "theme:language-server",
        hidden: false,
        dispatch: DispatchMode::Native,
    },
    CommandInfo {
        id: "theme:list",
        hidden: false,
        dispatch: DispatchMode::Native,
    },
    CommandInfo {
        id: "theme:metafields:pull",
        hidden: false,
        dispatch: DispatchMode::Native,
    },
    CommandInfo {
        id: "theme:open",
        hidden: false,
        dispatch: DispatchMode::Native,
    },
    CommandInfo {
        id: "theme:package",
        hidden: false,
        dispatch: DispatchMode::Native,
    },
    CommandInfo {
        id: "theme:preview",
        hidden: false,
        dispatch: DispatchMode::Native,
    },
    CommandInfo {
        id: "theme:profile",
        hidden: false,
        dispatch: DispatchMode::Native,
    },
    CommandInfo {
        id: "theme:publish",
        hidden: false,
        dispatch: DispatchMode::Native,
    },
    CommandInfo {
        id: "theme:pull",
        hidden: false,
        dispatch: DispatchMode::Native,
    },
    CommandInfo {
        id: "theme:push",
        hidden: false,
        dispatch: DispatchMode::Native,
    },
    CommandInfo {
        id: "theme:rename",
        hidden: false,
        dispatch: DispatchMode::Native,
    },
    CommandInfo {
        id: "theme:share",
        hidden: false,
        dispatch: DispatchMode::Native,
    },
    CommandInfo {
        id: "upgrade",
        hidden: false,
        dispatch: DispatchMode::Native,
    },
    CommandInfo {
        id: "version",
        hidden: false,
        dispatch: DispatchMode::Native,
    },
];

pub fn command_ids() -> impl Iterator<Item = &'static str> {
    COMMANDS.iter().map(|command| command.id)
}

pub fn visible_command_ids() -> impl Iterator<Item = &'static str> {
    COMMANDS
        .iter()
        .filter(|command| !command.hidden)
        .map(|command| command.id)
}

pub fn find(id: &str) -> Option<&'static CommandInfo> {
    COMMANDS.iter().find(|command| command.id == id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn registry_has_no_duplicates() {
        let mut seen = HashSet::new();
        for id in command_ids() {
            assert!(seen.insert(id), "duplicate command id: {id}");
        }
    }

    #[test]
    fn registry_covers_upstream_manifest_count() {
        assert_eq!(COMMANDS.len(), 115);
    }

    #[test]
    fn missing_surfaces_are_registered_as_bridges() {
        assert_eq!(find("hydrogen:dev").unwrap().dispatch, DispatchMode::Bridge);
        assert_eq!(
            find("hydrogen:setup:css").unwrap().dispatch,
            DispatchMode::Bridge
        );
        assert_eq!(
            find("plugins:install").unwrap().dispatch,
            DispatchMode::Bridge
        );
        assert_eq!(
            find("doctor-release:theme").unwrap().dispatch,
            DispatchMode::Bridge
        );
    }

    #[test]
    fn upstream_hidden_flags_are_recorded() {
        for id in [
            "cache:clear",
            "docs:generate",
            "hydrogen:g",
            "notifications:list",
            "plugins",
            "store:list",
        ] {
            assert!(find(id).unwrap().hidden, "{id} should be hidden");
        }
    }

    #[test]
    fn plugin_lifecycle_subcommands_match_upstream_visibility() {
        assert!(find("plugins").unwrap().hidden);
        for id in [
            "plugins:inspect",
            "plugins:install",
            "plugins:link",
            "plugins:reset",
            "plugins:uninstall",
            "plugins:update",
        ] {
            assert!(!find(id).unwrap().hidden, "{id} should be visible");
        }
    }
}
