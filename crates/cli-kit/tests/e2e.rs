use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
use tempfile::tempdir;

fn shopify() -> Command {
    Command::cargo_bin("shopify").unwrap()
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
        .stdout(predicate::str::contains("@shopify/cli/"));
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
fn did_you_mean_unknown_command() {
    shopify()
        .arg("deply")
        .assert()
        .failure()
        .stderr(predicate::str::contains("Did you mean").and(predicate::str::contains("deploy")));
}
