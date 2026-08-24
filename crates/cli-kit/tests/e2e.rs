use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
use tempfile::tempdir;

fn shopify() -> Command {
    Command::cargo_bin("shopify").unwrap()
}

fn create_app() -> Command {
    Command::cargo_bin("create-app").unwrap()
}

#[test]
fn help_smoke() {
    shopify()
        .arg("help")
        .assert()
        .success()
        .stdout(predicate::str::contains("Shopify"));
}

#[test]
fn version_smoke() {
    shopify()
        .arg("version")
        .assert()
        .success()
        .stdout(predicate::str::contains("@shopify/cli/"))
        .stdout(predicate::str::contains("Bridge CLI:"));
}

#[test]
fn create_app_help_lists_upstream_init_flags() {
    create_app()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("--flavor"))
        .stdout(predicate::str::contains("--organization-id"))
        .stdout(predicate::str::contains("--package-manager"));
}

#[test]
fn app_help() {
    shopify()
        .args(["app", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("deploy"));
}

#[test]
fn store_help() {
    shopify()
        .args(["store", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("store").or(predicate::str::contains("Store")));
}

#[test]
fn cache_clear() {
    shopify()
        .args(["cache", "clear"])
        .assert()
        .success()
        .stdout(predicate::str::contains("cleared"));
}

#[test]
fn config_autoupgrade_status() {
    shopify()
        .args(["config", "autoupgrade", "status"])
        .assert()
        .success()
        .stdout(predicate::str::contains("autoupgrade"));
}

#[test]
fn search_dev() {
    shopify()
        .args(["search", "dev"])
        .assert()
        .success()
        .stdout(predicate::str::contains("app dev"));
}

#[test]
fn app_config_validate_valid_toml() {
    let dir = tempdir().unwrap();
    fs::write(
        dir.path().join("shopify.app.toml"),
        "client_id = \"gid://app/1\"\nname = \"E2E\"\napplication_url = \"https://example.com\"\nembedded = true\n",
    )
    .unwrap();
    shopify()
        .args(["app", "config", "validate", "--path"])
        .arg(dir.path())
        .assert()
        .success();
}

#[test]
fn app_config_validate_invalid_toml() {
    let dir = tempdir().unwrap();
    fs::write(dir.path().join("shopify.app.toml"), "name = \n").unwrap();
    shopify()
        .args(["app", "config", "validate", "--path"])
        .arg(dir.path())
        .assert()
        .failure();
}

#[test]
fn theme_help() {
    shopify()
        .args(["theme", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("push").and(predicate::str::contains("dev")));
}

#[test]
fn hydrogen_help_lists_bridged_commands() {
    shopify()
        .args(["hydrogen", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("dev").and(predicate::str::contains("setup")));
}

#[test]
fn plugins_help_lists_oclif_compat_commands() {
    shopify()
        .args(["plugins", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("install").and(predicate::str::contains("update")));
}

#[test]
fn doctor_release_help_lists_hidden_theme_command() {
    shopify()
        .args(["doctor-release", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("theme"));
}

#[test]
fn doctor_release_theme_help_parses_without_bridge_runner() {
    shopify()
        .args(["doctor-release", "theme", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("theme"));
}

#[test]
fn commands_json_lists_bridge_dispatch() {
    shopify()
        .args(["commands", "--all", "--json"])
        .assert()
        .success()
        .stdout(predicate::str::contains("hydrogen:dev"))
        .stdout(predicate::str::contains("\"bridge\""));
}

#[test]
fn docs_generate_prints_command_reference() {
    shopify()
        .args(["docs", "generate"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Shopify CLI Commands"))
        .stdout(predicate::str::contains("shopify hydrogen dev"));
}

#[test]
fn search_includes_visible_hydrogen_commands() {
    shopify()
        .args(["search", "hydrogen dev"])
        .assert()
        .success()
        .stdout(predicate::str::contains("shopify hydrogen dev"));
}

#[cfg(unix)]
#[test]
fn bridged_hydrogen_command_invokes_runner() {
    use std::os::unix::fs::PermissionsExt;

    let dir = tempdir().unwrap();
    let log = dir.path().join("bridge.log");
    let runner = dir.path().join("bridge.sh");
    fs::write(
        &runner,
        format!(
            "#!/bin/sh\nprintf '%s\\n' \"$SHOPIFY_FLAG_PATH\" \"$@\" > '{}'\n",
            log.display()
        ),
    )
    .unwrap();
    let mut perms = fs::metadata(&runner).unwrap().permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&runner, perms).unwrap();

    shopify()
        .env("SHOPIFY_CLI_BRIDGE_RUNNER", &runner)
        .args(["--path", "web", "hydrogen", "dev", "--shop", "demo"])
        .assert()
        .success();

    let raw = fs::read_to_string(log).unwrap();
    assert_eq!(raw, "web\nhydrogen:dev\n--shop\ndemo\n");
}

#[test]
fn did_you_mean_unknown_command() {
    shopify()
        .arg("deply")
        .assert()
        .failure()
        .stderr(predicate::str::contains("Did you mean").and(predicate::str::contains("deploy")));
}
