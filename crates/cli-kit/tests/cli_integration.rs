use assert_cmd::Command;
use predicates::prelude::*;

fn cli() -> Command {
    Command::cargo_bin("shopify").unwrap()
}

#[test]
fn test_help_flag() {
    cli()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "A CLI tool to build for the Shopify platform",
        ))
        .stdout(predicate::str::contains("auth"))
        .stdout(predicate::str::contains("organization"));
}

#[test]
fn test_version_flag() {
    cli()
        .arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::contains("shopify"));
}

#[test]
fn test_help_command() {
    cli()
        .arg("help")
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "A CLI tool to build for the Shopify platform",
        ));
}

#[test]
fn test_version_command() {
    cli()
        .arg("version")
        .assert()
        .success()
        .stdout(predicate::str::contains("shopify"));
}

#[test]
fn test_auth_help() {
    cli()
        .args(["auth", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("login"))
        .stdout(predicate::str::contains("logout"));
}

#[test]
fn test_auth_login_help() {
    cli()
        .args(["auth", "login", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Login to Shopify"));
}

#[test]
fn test_auth_logout_help() {
    cli()
        .args(["auth", "logout", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Logout from Shopify"));
}

#[test]
fn test_auth_status_help() {
    cli()
        .args(["auth", "status", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("authentication status"));
}

#[test]
fn test_organization_help() {
    cli()
        .args(["organization", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("list"));
}

#[test]
fn test_organization_list_help() {
    cli()
        .args(["organization", "list", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("List the organizations"))
        .stdout(predicate::str::contains("--json"));
}

#[test]
fn test_organization_list_json_flag() {
    cli()
        .args(["organization", "list", "--json", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("List the organizations"));
}

#[test]
fn test_unknown_command() {
    cli()
        .arg("nonexistent")
        .assert()
        .failure()
        .stderr(predicate::str::contains("error"));
}

#[test]
fn test_unknown_auth_command() {
    cli()
        .args(["auth", "nonexistent"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("error"));
}

#[test]
fn test_app_help_lists_new_commands() {
    cli()
        .args(["app", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("dev"))
        .stdout(predicate::str::contains("env"))
        .stdout(predicate::str::contains("webhook"))
        .stdout(predicate::str::contains("logs"));
}

#[test]
fn test_app_dev_help() {
    cli()
        .args(["app", "dev", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("use-localhost"))
        .stdout(predicate::str::contains("tunnel-url"));
}

#[test]
fn test_app_dev_clean_help() {
    cli()
        .args(["app", "dev", "clean", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("clean").or(predicate::str::contains("Clean")));
}

#[test]
fn test_app_env_help() {
    cli()
        .args(["app", "env", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("pull"))
        .stdout(predicate::str::contains("show"));
}

#[test]
fn test_app_webhook_trigger_help() {
    cli()
        .args(["app", "webhook", "trigger", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("topic"))
        .stdout(predicate::str::contains("address"));
}

#[test]
fn test_app_logs_help() {
    cli()
        .args(["app", "logs", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("store").or(predicate::str::contains("sources")));
}

#[test]
fn test_app_logs_sources_help() {
    cli()
        .args(["app", "logs", "sources", "--help"])
        .assert()
        .success();
}
