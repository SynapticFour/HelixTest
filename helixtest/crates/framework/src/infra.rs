// SPDX-License-Identifier: Apache-2.0
//! Ferrum + ga4gh-infra co-deploy checks (opt-in; use `--mode ferrum+infra`).

use common::config::TestConfig;
use common::http::HttpClient;
use common::report::{
    ComplianceLevel, OverallReport, ServiceKind, ServiceReport, TestCaseResult, TestCategory,
};
use reqwest::header::{ACCEPT, COOKIE, SET_COOKIE};
use tracing::info;

fn env_or(key: &str, default: &str) -> String {
    std::env::var(key).unwrap_or_else(|_| default.to_string())
}

fn broker_url() -> String {
    env_or("GA4GH_BROKER_URL", "http://127.0.0.1:8180")
        .trim_end_matches('/')
        .to_string()
}

fn service_registry_url() -> String {
    env_or("GA4GH_SERVICE_REGISTRY_URL", "http://localhost:8183")
        .trim_end_matches('/')
        .to_string()
}

fn gateway_base(cfg: &TestConfig) -> String {
    if let Ok(v) = std::env::var("GATEWAY_BASE") {
        return v.trim_end_matches('/').to_string();
    }
    let drs = cfg.services.drs_url.trim_end_matches('/');
    if let Some(idx) = drs.find("/ga4gh/drs") {
        return drs[..idx].to_string();
    }
    "http://localhost:18080".into()
}

fn pass(name: &str, category: TestCategory) -> TestCaseResult {
    TestCaseResult::pass(name, ComplianceLevel::Level2, category)
}

fn fail(name: &str, category: TestCategory, err: impl std::fmt::Display) -> TestCaseResult {
    TestCaseResult::fail(name, ComplianceLevel::Level2, category, err)
}

async fn broker_login(client: &HttpClient) -> anyhow::Result<(String, String)> {
    let broker = broker_url();
    let login = client
        .inner()
        .get(format!("{broker}/login"))
        .header(ACCEPT, "application/json")
        .send()
        .await?;
    anyhow::ensure!(
        login.status().is_success(),
        "broker login HTTP {}",
        login.status()
    );

    let session_cookie = login
        .headers()
        .get_all(SET_COOKIE)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .find(|value| value.starts_with("ga4gh_broker_rp_session="))
        .ok_or_else(|| anyhow::anyhow!("missing broker session cookie"))?
        .split(';')
        .next()
        .unwrap_or_default()
        .to_string();

    let auth_url = login.json::<serde_json::Value>().await?["authorization_url"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("missing authorization_url"))?
        .replace("mock-idp:9100", "127.0.0.1:9100")
        .replace("mock-idp:9000", "127.0.0.1:9100");

    let auth_redirect = client.inner().get(&auth_url).send().await?;
    anyhow::ensure!(
        auth_redirect.status().is_redirection(),
        "mock-idp authorize expected redirect, got {}",
        auth_redirect.status()
    );
    let callback_url = auth_redirect
        .headers()
        .get("location")
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| anyhow::anyhow!("missing callback location"))?
        .to_string();

    let callback = client
        .inner()
        .get(&callback_url)
        .header(ACCEPT, "application/json")
        .header(COOKIE, session_cookie)
        .send()
        .await?;
    anyhow::ensure!(
        callback.status().is_success(),
        "broker callback HTTP {}",
        callback.status()
    );
    let callback_json = callback.json::<serde_json::Value>().await?;
    let passport = callback_json["access_token"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("missing access_token in callback"))?
        .to_string();
    let subject = env_or("MOCK_IDP_SUBJECT", "researcher@uni-heidelberg.de");
    Ok((subject, passport))
}

pub async fn run_infra(config_profile: Option<&str>) -> anyhow::Result<OverallReport> {
    let cfg = TestConfig::load(config_profile)?;
    let client = HttpClient::new();
    let base = gateway_base(&cfg);
    info!(%base, broker = %broker_url(), registry = %service_registry_url(), "Running HelixTest ferrum+infra mode");

    let mut tests = Vec::new();

    match client
        .get_json(&format!("{}/service-info", broker_url()))
        .await
    {
        Ok(v) if v.get("id").is_some() => {
            tests.push(pass("infra: broker service-info", TestCategory::Schema));
        }
        Ok(_) => tests.push(fail(
            "infra: broker service-info",
            TestCategory::Schema,
            "missing id field",
        )),
        Err(e) => tests.push(fail(
            "infra: broker service-info",
            TestCategory::Schema,
            format!("ga4gh-infra broker not reachable: {e}"),
        )),
    }

    match client
        .get_json(&format!("{}/services", service_registry_url()))
        .await
    {
        Ok(v) if v.as_array().map(|a| !a.is_empty()).unwrap_or(false) => {
            tests.push(pass(
                "infra: service registry lists entries",
                TestCategory::Interoperability,
            ));
            let has_drs = v.as_array().map(|entries| {
                entries.iter().any(|entry| {
                    let info = entry.get("info").unwrap_or(entry);
                    info.get("type")
                        .and_then(|t| t.get("artifact"))
                        .and_then(|a| a.as_str())
                        .map(|a| {
                            a.eq_ignore_ascii_case("drs") || a.eq_ignore_ascii_case("drsservice")
                        })
                        .unwrap_or(false)
                })
            });
            if has_drs == Some(true) {
                tests.push(pass(
                    "infra: Ferrum DRS registered in service registry",
                    TestCategory::Interoperability,
                ));
            } else {
                tests.push(fail(
                    "infra: Ferrum DRS registered in service registry",
                    TestCategory::Interoperability,
                    format!("no drs artifact in registry: {v}"),
                ));
            }
        }
        Ok(v) => tests.push(fail(
            "infra: service registry lists entries",
            TestCategory::Interoperability,
            format!("expected non-empty registry, got {v}"),
        )),
        Err(e) => tests.push(fail(
            "infra: service registry lists entries",
            TestCategory::Interoperability,
            format!("service registry not reachable: {e}"),
        )),
    }

    let object_id =
        std::env::var("HELIXTEST_AUTH_OBJECT_ID").unwrap_or_else(|_| "test-object-1".to_string());
    match broker_login(&client).await {
        Ok((_subject, passport)) => {
            tests.push(pass(
                "infra: broker login issues Passport",
                TestCategory::Security,
            ));
            let url = format!(
                "{}/ga4gh/drs/v1/objects/{}",
                base.trim_end_matches('/'),
                object_id
            );
            let resp = client.inner().get(&url).bearer_auth(&passport).send().await;
            match resp {
                Ok(r) if r.status().is_success() => tests.push(pass(
                    "infra: Passport accepted on Ferrum DRS",
                    TestCategory::Security,
                )),
                Ok(r) => tests.push(fail(
                    "infra: Passport accepted on Ferrum DRS",
                    TestCategory::Security,
                    format!("HTTP {} for DRS with broker Passport", r.status()),
                )),
                Err(e) => tests.push(fail(
                    "infra: Passport accepted on Ferrum DRS",
                    TestCategory::Security,
                    e,
                )),
            }
        }
        Err(e) => tests.push(fail(
            "infra: broker login issues Passport",
            TestCategory::Security,
            format!("broker login flow unavailable: {e}"),
        )),
    }

    Ok(OverallReport {
        services: vec![ServiceReport {
            service: ServiceKind::Infra,
            tests,
        }],
        skipped_services: vec![],
        executed_test_modules: vec![ServiceKind::Infra],
        enabled_services: vec![ServiceKind::Infra],
        diagnostics: None,
    })
}
