// SPDX-License-Identifier: Apache-2.0
//! HelixTest DRS checks against an in-process mock DRS (not Ferrum).

use std::collections::HashSet;

use common::config::{AuthChecksConfig, ServiceConfig, SubsetConfig, TestConfig};
use common::http::HttpClient;
use common::report::{ServiceKind, TestStatus};
use framework::drs::{
    run_drs_checks, run_drs_checks_with_fixture, unknown_object_id_for, DrsTestFixture,
    FIXTURE_UNAVAILABLE,
};
use framework::{run_all, Features, Mode};

#[path = "../../../testing/mock_ga4gh_drs.rs"]
mod mock_ga4gh_drs;

use mock_ga4gh_drs::{start_mock_ga4gh_drs, DRS_CHECK_NAMES};

fn drs_cfg(drs_url: &str, wes_url: &str) -> TestConfig {
    TestConfig {
        services: ServiceConfig {
            wes_url: wes_url.to_string(),
            tes_url: String::new(),
            drs_url: drs_url.to_string(),
            trs_url: String::new(),
            beacon_url: String::new(),
            auth_url: String::new(),
            htsget_url: None,
        },
        subset: SubsetConfig::default(),
        auth_checks: AuthChecksConfig::default(),
    }
}

fn assert_same_drs_checks(names: &[String]) {
    assert_eq!(
        names, DRS_CHECK_NAMES,
        "DRS check set must match Ferrum-target DRS checks"
    );
}

#[tokio::test]
async fn drs_checks_pass_against_non_ferrum_mock() {
    let mock = start_mock_ga4gh_drs().await;
    let cfg = drs_cfg(&mock.drs_url(), &mock.drs_url());
    let client = HttpClient::new();
    let features = Features {
        strict_drs_checksums: true,
        ..Features::default()
    };
    let report = run_drs_checks(Mode::Generic, &features, &cfg, &client)
        .await
        .expect("DRS checks against mock DRS");
    let names: Vec<String> = report.tests.iter().map(|t| t.name.clone()).collect();
    assert_same_drs_checks(&names);
    for t in &report.tests {
        assert_eq!(
            t.status,
            TestStatus::Pass,
            "check {} failed: {:?}",
            t.name,
            t.error
        );
    }
}

#[tokio::test]
async fn generic_mode_does_not_follow_wes_name_ferrum() {
    let mock = start_mock_ga4gh_drs().await;
    let drs_url = mock.drs_url();
    // SAFETY: this integration-test binary only mutates these keys in this test.
    unsafe {
        std::env::remove_var("HELIXTEST_PROFILE");
        std::env::remove_var("HELIXTEST_CONFIG");
        std::env::set_var("DRS_URL", &drs_url);
        std::env::set_var("WES_URL", &drs_url);
    }
    let only = Some(HashSet::from([ServiceKind::Drs]));
    let report = run_all(Mode::Generic, only, None)
        .await
        .expect("generic DRS run");
    unsafe {
        std::env::remove_var("DRS_URL");
        std::env::remove_var("WES_URL");
    }
    let drs = report
        .services
        .iter()
        .find(|s| s.service == ServiceKind::Drs)
        .expect("DRS service report");
    let checksum = drs
        .tests
        .iter()
        .find(|t| t.name == "DRS checksum correctness")
        .expect("checksum check");
    assert_eq!(
        checksum.status,
        TestStatus::Skip,
        "generic mode must not load Ferrum features just because WES name contains Ferrum; got {:?}",
        checksum.error
    );
}

#[test]
fn unknown_object_id_is_deterministic_and_not_the_positive_fixture() {
    let a = unknown_object_id_for("test-object-1");
    let b = unknown_object_id_for("test-object-1");
    let c = unknown_object_id_for("other-object");
    assert_eq!(a, b);
    assert_ne!(a, "test-object-1");
    assert_ne!(a, c);
    assert!(a.starts_with("helix.unknown."));
    assert!(!a.contains("test-object-1"));
}

#[tokio::test]
async fn missing_configured_object_skips_as_fixture_unavailable() {
    let mock = start_mock_ga4gh_drs().await;
    let cfg = drs_cfg(&mock.drs_url(), &mock.drs_url());
    let client = HttpClient::new();
    let features = Features {
        strict_drs_checksums: true,
        ..Features::default()
    };
    let fixture = DrsTestFixture {
        object_id: "absent-object".into(),
        expected_sha256: None,
    };
    let report = run_drs_checks_with_fixture(Mode::Generic, &features, &cfg, &client, &fixture)
        .await
        .expect("DRS checks");
    for t in &report.tests {
        if t.name == "DRS invalid object id returns 404" {
            assert_eq!(t.status, TestStatus::Pass, "{:?}", t.error);
        } else {
            assert_eq!(t.status, TestStatus::Skip, "{} {:?}", t.name, t.error);
            let msg = t.error.as_deref().unwrap_or("");
            assert!(
                msg.contains(FIXTURE_UNAVAILABLE),
                "{} missing fixture_unavailable: {msg}",
                t.name
            );
        }
    }
}
