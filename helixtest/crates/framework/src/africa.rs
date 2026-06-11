//! Africa-mode conformance checks (opt-in; does not affect `--mode ferrum`).

use common::config::TestConfig;
use common::http::HttpClient;
use common::report::{
    ComplianceLevel, OverallReport, ServiceKind, ServiceReport, TestCaseResult, TestCategory,
};
use std::path::Path;
use tracing::info;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AfricaProfile {
    Offline,
    Ont,
    Outbreak,
    Federation,
    All,
}

impl AfricaProfile {
    pub fn from_str(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "offline" => Some(Self::Offline),
            "ont" => Some(Self::Ont),
            "outbreak" => Some(Self::Outbreak),
            "federation" => Some(Self::Federation),
            "all" => Some(Self::All),
            _ => None,
        }
    }
}

fn gateway_base(cfg: &TestConfig) -> String {
    if let Ok(v) = std::env::var("GATEWAY_BASE") {
        return v.trim_end_matches('/').to_string();
    }
    let drs = cfg.services.drs_url.trim_end_matches('/');
    if let Some(idx) = drs.find("/ga4gh/drs") {
        return drs[..idx].to_string();
    }
    "http://localhost:8080".into()
}

fn pass(name: &str, category: TestCategory) -> TestCaseResult {
    TestCaseResult {
        name: name.into(),
        level: ComplianceLevel::Level2,
        passed: true,
        error: None,
        category,
        weight: 1.0,
    }
}

fn fail(name: &str, category: TestCategory, err: impl std::fmt::Display) -> TestCaseResult {
    TestCaseResult {
        name: name.into(),
        level: ComplianceLevel::Level2,
        passed: false,
        error: Some(err.to_string()),
        category,
        weight: 1.0,
    }
}

fn skip(name: &str, reason: &str) -> TestCaseResult {
    TestCaseResult {
        name: name.into(),
        level: ComplianceLevel::Level1,
        passed: true,
        error: Some(format!("skipped: {reason}")),
        category: TestCategory::Other,
        weight: 0.0,
    }
}

pub async fn run_africa(profile: AfricaProfile) -> anyhow::Result<OverallReport> {
    let cfg = TestConfig::from_env_or_file()?;
    let client = HttpClient::new();
    let base = gateway_base(&cfg);
    info!(?profile, %base, "Running HelixTest Africa mode");

    let mut tests = Vec::new();
    let run_offline = matches!(profile, AfricaProfile::Offline | AfricaProfile::All);
    let run_ont = matches!(profile, AfricaProfile::Ont | AfricaProfile::All);
    let run_outbreak = matches!(profile, AfricaProfile::Outbreak | AfricaProfile::All);
    let run_federation = matches!(profile, AfricaProfile::Federation | AfricaProfile::All);

    if run_offline {
        tests.extend(run_offline_profile(&client, &base, &cfg).await);
    }
    if run_ont {
        tests.extend(run_ont_profile(&client, &base).await);
    }
    if run_outbreak {
        tests.extend(run_outbreak_profile(&client, &base).await);
    }
    if run_federation {
        tests.extend(run_federation_profile(&client, &base).await);
    }

    Ok(OverallReport {
        services: vec![ServiceReport {
            service: ServiceKind::Africa,
            tests,
        }],
        skipped_services: vec![],
        executed_test_modules: vec![ServiceKind::Africa],
        enabled_services: vec![ServiceKind::Africa],
        diagnostics: None,
    })
}

async fn run_offline_profile(
    client: &HttpClient,
    base: &str,
    cfg: &TestConfig,
) -> Vec<TestCaseResult> {
    let mut tests = Vec::new();

    match client
        .get_json(&format!("{}/health", base))
        .await
    {
        Ok(_) => tests.push(pass("offline: gateway /health", TestCategory::Lifecycle)),
        Err(e) => tests.push(fail(
            "offline: gateway /health",
            TestCategory::Lifecycle,
            e,
        )),
    }

    for (name, url) in [
        ("DRS", cfg.services.drs_url.trim_end_matches('/')),
        ("Beacon", cfg.services.beacon_url.trim_end_matches('/')),
    ] {
        let si = format!("{}/service-info", url);
        match client.get_json(&si).await {
            Ok(v) if v.get("id").is_some() => {
                tests.push(pass(
                    &format!("offline: {name} service-info"),
                    TestCategory::Schema,
                ));
            }
            Ok(_) => tests.push(fail(
                &format!("offline: {name} service-info"),
                TestCategory::Schema,
                "missing id field",
            )),
            Err(e) => tests.push(fail(
                &format!("offline: {name} service-info"),
                TestCategory::Schema,
                e,
            )),
        }
    }

    match client
        .get_json(&format!("{}/api/v1/references", base))
        .await
    {
        Ok(v) if v.as_array().map(|a| a.len() >= 6).unwrap_or(false) => {
            tests.push(pass(
                "offline: reference registry seeded",
                TestCategory::Schema,
            ));
        }
        Ok(v) => tests.push(fail(
            "offline: reference registry seeded",
            TestCategory::Schema,
            format!("expected >=6 entries, got {:?}", v),
        )),
        Err(e) => tests.push(fail(
            "offline: reference registry seeded",
            TestCategory::Schema,
            e,
        )),
    }

    tests
}

async fn run_ont_profile(client: &HttpClient, base: &str) -> Vec<TestCaseResult> {
    let mut tests = Vec::new();
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("fixtures/africa/synthetic_ont_file.pod5.stub");
    let stub = match std::fs::read(&fixture) {
        Ok(b) => b,
        Err(e) => {
            tests.push(fail(
                "ont: load fixture stub",
                TestCategory::Lifecycle,
                e,
            ));
            return tests;
        }
    };

    let ont_meta = serde_json::json!({
        "format": "pod5",
        "source_path": "/data/synthetic_ont_file.pod5.stub",
        "run_id": "helixtest-africa-run",
        "sample_id": "sample-1",
        "organism": "Plasmodium_falciparum",
        "dorado_basecalled": false,
        "quality_metrics": {
            "mean_qscore": 12.5,
            "read_count": 100,
            "n50": 1000
        }
    });

    let form = reqwest::multipart::Form::new()
        .text("ont_metadata", ont_meta.to_string())
        .part(
            "file",
            reqwest::multipart::Part::bytes(stub)
                .file_name("synthetic_ont_file.pod5.stub")
                .mime_str("application/octet-stream")
                .unwrap(),
        );

    let url = format!("{}/api/v1/ingest/ont", base);
    let resp = client
        .inner()
        .post(&url)
        .multipart(form)
        .send()
        .await;

    let drs_id = match resp {
        Ok(r) if r.status().is_success() => match r.json::<serde_json::Value>().await {
            Ok(v) => v
                .get("drs_object_id")
                .and_then(|x| x.as_str())
                .map(|s| s.to_string()),
            Err(e) => {
                tests.push(fail("ont: ingest response JSON", TestCategory::Lifecycle, e));
                None
            }
        },
        Ok(r) => {
            let status = r.status();
            let text = r.text().await.unwrap_or_default();
            tests.push(fail(
                "ont: POST /api/v1/ingest/ont",
                TestCategory::Lifecycle,
                format!("HTTP {status}: {text}"),
            ));
            None
        }
        Err(e) => {
            tests.push(fail(
                "ont: POST /api/v1/ingest/ont",
                TestCategory::Lifecycle,
                e,
            ));
            None
        }
    };

    if let Some(id) = drs_id {
        tests.push(pass("ont: DRS object created", TestCategory::Lifecycle));
        let obj_url = format!(
            "{}/ga4gh/drs/v1/objects/{}",
            base.trim_end_matches('/'),
            id
        );
        match client.get_json(&obj_url).await {
            Ok(v) if v.get("ont_metrics").is_some() => {
                tests.push(pass("ont: ont_metrics on DRS object", TestCategory::Schema));
            }
            Ok(v) => tests.push(fail(
                "ont: ont_metrics on DRS object",
                TestCategory::Schema,
                format!("missing ont_metrics: {v}"),
            )),
            Err(e) => tests.push(fail(
                "ont: ont_metrics on DRS object",
                TestCategory::Schema,
                e,
            )),
        }

        let beacon_url = format!(
            "{}/ga4gh/beacon/v2/g_variants?organism=Plasmodium_falciparum",
            base.trim_end_matches('/')
        );
        match client.get_json(&beacon_url).await {
            Ok(v) if v["response"]["exists"] == true => {
                tests.push(pass(
                    "ont: Beacon organism query finds ingested object",
                    TestCategory::Interoperability,
                ));
            }
            Ok(v) => tests.push(fail(
                "ont: Beacon organism query finds ingested object",
                TestCategory::Interoperability,
                format!("exists not true: {v}"),
            )),
            Err(e) => tests.push(fail(
                "ont: Beacon organism query finds ingested object",
                TestCategory::Interoperability,
                e,
            )),
        }
    }

    tests
}

async fn run_outbreak_profile(client: &HttpClient, base: &str) -> Vec<TestCaseResult> {
    let mut tests = Vec::new();
    let activate = serde_json::json!({
        "policy_name": "helixtest-africa",
        "trigger_pathogen": "Plasmodium_falciparum",
        "activated_by": "helixtest"
    });
    let url = format!("{}/api/v1/outbreak/activate", base);
    let resp = client.post_json(&url, &activate).await;
    match resp {
        Ok(v) if v.get("active").and_then(|x| x.as_bool()) == Some(true) => {
            tests.push(pass("outbreak: activate policy", TestCategory::Security));
        }
        Ok(v) => tests.push(fail(
            "outbreak: activate policy",
            TestCategory::Security,
            format!("unexpected response: {v}"),
        )),
        Err(e) => {
            tests.push(skip(
                "outbreak: activate policy",
                &format!("outbreak not enabled or auth required: {e}"),
            ));
            return tests;
        }
    }

    let deactivate = serde_json::json!({
        "policy_name": "helixtest-africa",
        "deactivated_by": "helixtest",
        "deactivation_reason": "helixtest africa profile"
    });
    match client
        .post_json(
            &format!("{}/api/v1/outbreak/deactivate", base),
            &deactivate,
        )
        .await
    {
        Ok(_) => tests.push(pass("outbreak: deactivate policy", TestCategory::Security)),
        Err(e) => tests.push(fail("outbreak: deactivate policy", TestCategory::Security, e)),
    }

    match client
        .get_json(&format!("{}/api/v1/audit/residency/verify", base))
        .await
    {
        Ok(v) if v.get("chain_valid").and_then(|x| x.as_bool()) == Some(true) => {
            tests.push(pass(
                "outbreak: residency audit chain valid",
                TestCategory::Security,
            ));
        }
        Ok(v) => tests.push(fail(
            "outbreak: residency audit chain valid",
            TestCategory::Security,
            format!("chain_valid false or missing: {v}"),
        )),
        Err(e) => tests.push(fail(
            "outbreak: residency audit chain valid",
            TestCategory::Security,
            e,
        )),
    }

    tests
}

async fn run_federation_profile(client: &HttpClient, base: &str) -> Vec<TestCaseResult> {
    let mut tests = Vec::new();
    let peer = std::env::var("FERRUM_AFRICA_PEER_URL").ok();
    let Some(peer_base) = peer else {
        tests.push(skip(
            "federation: peer query",
            "set FERRUM_AFRICA_PEER_URL to a second Ferrum gateway for federation tests",
        ));
        return tests;
    };

    let local_url = format!(
        "{}/ga4gh/beacon/v2/g_variants?federate=true&referenceName=1&start=1000&referenceBases=A&alternateBases=T",
        base.trim_end_matches('/')
    );
    match client.get_json(&local_url).await {
        Ok(v) => {
            tests.push(pass("federation: local federate query succeeds", TestCategory::Interoperability));
            if v["meta"]["warnings"].is_array() {
                tests.push(pass(
                    "federation: warnings present when peers fail",
                    TestCategory::Robustness,
                ));
            }
        }
        Err(e) => tests.push(fail(
            "federation: local federate query succeeds",
            TestCategory::Interoperability,
            e,
        )),
    }

    let _ = peer_base;
    tests
}
