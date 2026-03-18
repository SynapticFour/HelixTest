use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn help_includes_project_name() {
    Command::cargo_bin("helixtest")
        .unwrap()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("HelixTest"));
}

#[test]
fn version_prints() {
    Command::cargo_bin("helixtest")
        .unwrap()
        .arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::contains("helixtest"));
}

#[test]
fn running_without_all_is_noop() {
    Command::cargo_bin("helixtest")
        .unwrap()
        .assert()
        .success()
        .stdout(predicate::str::contains("Nothing to do"));
}

#[test]
fn invalid_enum_argument_fails() {
    Command::cargo_bin("helixtest")
        .unwrap()
        .args(["--report", "nope"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("possible values"));
}

