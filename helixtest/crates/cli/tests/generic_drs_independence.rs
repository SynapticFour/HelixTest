// SPDX-License-Identifier: Apache-2.0
//! `helixtest` against an in-process mock DRS (not Ferrum). Same DRS checks as Ferrum.

use std::time::Duration;

use assert_cmd::Command;
use serde_json::Value;

#[path = "../../../testing/mock_ga4gh_drs.rs"]
mod mock_ga4gh_drs;

use mock_ga4gh_drs::{start_mock_ga4gh_drs, DRS_CHECK_NAMES};

fn helixtest() -> Command {
    let mut cmd = Command::cargo_bin("helixtest").expect("helixtest binary");
    cmd.env_remove("HELIXTEST_PROFILE")
        .env_remove("HELIXTEST_CONFIG")
        .env("RUST_LOG", "error")
        .timeout(Duration::from_secs(60));
    cmd
}

fn drs_tests(report: &Value) -> &[Value] {
    let services = report
        .get("services")
        .and_then(|s| s.as_array())
        .expect("services array");
    let drs = services
        .iter()
        .find(|s| s.get("service").and_then(|v| v.as_str()) == Some("Drs"))
        .expect("DRS service in report");
    drs.get("tests")
        .and_then(|t| t.as_array())
        .map(|a| a.as_slice())
        .expect("DRS tests")
}

fn assert_same_drs_checks(tests: &[Value]) {
    let names: Vec<&str> = tests
        .iter()
        .map(|t| t.get("name").and_then(|n| n.as_str()).unwrap_or(""))
        .collect();
    assert_eq!(
        names, DRS_CHECK_NAMES,
        "CLI DRS check set must match framework DRS checks"
    );
}

#[tokio::test]
async fn helixtest_cli_passes_same_drs_checks_against_mock() {
    let mock = start_mock_ga4gh_drs().await;
    let url = mock.drs_url();

    let assert = helixtest()
        .env("DRS_URL", &url)
        .env("WES_URL", &url)
        .args([
            "--all",
            "--mode",
            "generic",
            "--only",
            "drs",
            "--profile",
            "ga4gh-drs",
            "--report",
            "json",
        ])
        .assert()
        .success();
    let stdout = String::from_utf8_lossy(&assert.get_output().stdout);
    let report: Value = serde_json::from_str(stdout.trim()).unwrap_or_else(|e| {
        panic!("CLI JSON report parse failed: {e}; stdout={stdout}");
    });
    let tests = drs_tests(&report);
    assert_same_drs_checks(tests);
    for t in tests {
        let status = t.get("status").and_then(|s| s.as_str()).unwrap_or("");
        assert_eq!(
            status,
            "pass",
            "check {} failed: {:?}",
            t.get("name").and_then(|n| n.as_str()).unwrap_or("?"),
            t.get("error")
        );
    }
}

#[tokio::test]
async fn helixtest_generic_ignores_wes_service_info_name_ferrum() {
    let mock = start_mock_ga4gh_drs().await;
    let url = mock.drs_url();

    let assert = helixtest()
        .env("DRS_URL", &url)
        .env("WES_URL", &url)
        .args([
            "--all", "--mode", "generic", "--only", "drs", "--report", "json",
        ])
        .assert()
        .success();
    let stdout = String::from_utf8_lossy(&assert.get_output().stdout);
    let report: Value = serde_json::from_str(stdout.trim()).unwrap_or_else(|e| {
        panic!("CLI JSON report parse failed: {e}; stdout={stdout}");
    });
    let tests = drs_tests(&report);
    assert_same_drs_checks(tests);
    let checksum = tests
        .iter()
        .find(|t| t.get("name").and_then(|n| n.as_str()) == Some("DRS checksum correctness"))
        .expect("checksum check");
    assert_eq!(
        checksum.get("status").and_then(|s| s.as_str()),
        Some("skip"),
        "generic without ga4gh-drs/ferrum profile must not inherit Ferrum features from WES name; {:?}",
        checksum.get("error")
    );
    for t in tests {
        let name = t.get("name").and_then(|n| n.as_str()).unwrap_or("");
        if name == "DRS checksum correctness" {
            continue;
        }
        assert_eq!(
            t.get("status").and_then(|s| s.as_str()),
            Some("pass"),
            "check {name} failed: {:?}",
            t.get("error")
        );
    }
}
