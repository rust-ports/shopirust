use assert_cmd::Command;
use predicates::prelude::*;

fn cli() -> Command {
    Command::cargo_bin("cli-kit").unwrap()
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
