//! Level 4 (Security) conformance: GA4GH Passports / JWT auth (valid token, expired, scope, missing).

use anyhow::Result;
use chrono::Duration;
use common::auth::build_jwt;
use common::config::TestConfig;
use common::http::HttpClient;
use common::report::{ComplianceLevel, ServiceKind, ServiceReport, TestCaseResult, TestCategory};

use crate::{Features, Mode};

fn auth_secret() -> String {
    std::env::var("HELIXTEST_SHARED_SECRET").unwrap_or_else(|_| "test-secret".to_owned())
}

/// Test object ID used for auth checks (DRS GET with Bearer). Override via HELIXTEST_AUTH_OBJECT_ID.
fn auth_test_object_id() -> String {
    std::env::var("HELIXTEST_AUTH_OBJECT_ID").unwrap_or_else(|_| "test-object-1".to_owned())
}

pub async fn run_auth_checks(
    _mode: Mode,
    _features: &Features,
    cfg: &TestConfig,
    client: &HttpClient,
) -> Result<ServiceReport> {
    let mut tests = Vec::new();
    tests.push(level4_valid_token_grants_access(cfg, client).await);
    tests.push(level4_expired_token_rejected(cfg, client).await);
    tests.push(level4_wrong_scope_denied(cfg, client).await);
    tests.push(level4_missing_token_returns_401(cfg, client).await);

    Ok(ServiceReport {
        service: ServiceKind::Auth,
        tests,
    })
}

async fn level4_valid_token_grants_access(cfg: &TestConfig, client: &HttpClient) -> TestCaseResult {
    let result = async {
        let token = build_jwt(
            "https://auth.ga4gh.test",
            "test-user",
            "drs",
            "drs.read",
            Duration::minutes(5),
            &auth_secret(),
        )?;
        let url = format!(
            "{}/objects/{}",
            cfg.services.drs_url.trim_end_matches('/'),
            auth_test_object_id()
        );
        let resp = client.inner().get(&url).bearer_auth(&token).send().await?;
        anyhow::ensure!(
            resp.status().is_success(),
            "Valid token should be accepted, got {}",
            resp.status()
        );
        Ok::<(), anyhow::Error>(())
    }
    .await;

    TestCaseResult {
        name: "Auth: valid token grants access".into(),
        level: ComplianceLevel::Level4,
        passed: result.is_ok(),
        error: result.err().map(|e| e.to_string()),
        category: TestCategory::Security,
        weight: 1.0,
    }
}

async fn level4_expired_token_rejected(cfg: &TestConfig, client: &HttpClient) -> TestCaseResult {
    let result = async {
        let token = build_jwt(
            "https://auth.ga4gh.test",
            "test-user",
            "drs",
            "drs.read",
            Duration::minutes(-5),
            &auth_secret(),
        )?;
        let url = format!(
            "{}/objects/{}",
            cfg.services.drs_url.trim_end_matches('/'),
            auth_test_object_id()
        );
        let resp = client.inner().get(&url).bearer_auth(&token).send().await?;
        anyhow::ensure!(
            resp.status().is_client_error(),
            "Expired token must be rejected with 4xx, got {}",
            resp.status()
        );
        Ok::<(), anyhow::Error>(())
    }
    .await;

    TestCaseResult {
        name: "Auth: expired token rejected".into(),
        level: ComplianceLevel::Level4,
        passed: result.is_ok(),
        error: result.err().map(|e| e.to_string()),
        category: TestCategory::Security,
        weight: 1.0,
    }
}

async fn level4_wrong_scope_denied(cfg: &TestConfig, client: &HttpClient) -> TestCaseResult {
    let result = async {
        let token = build_jwt(
            "https://auth.ga4gh.test",
            "test-user",
            "drs",
            "wes.run",
            Duration::minutes(5),
            &auth_secret(),
        )?;
        let url = format!(
            "{}/objects/{}",
            cfg.services.drs_url.trim_end_matches('/'),
            auth_test_object_id()
        );
        let resp = client.inner().get(&url).bearer_auth(&token).send().await?;
        anyhow::ensure!(
            resp.status() == 403 || resp.status() == 401,
            "Wrong scope must deny access (403/401), got {}",
            resp.status()
        );
        Ok::<(), anyhow::Error>(())
    }
    .await;

    TestCaseResult {
        name: "Auth: wrong scope denied".into(),
        level: ComplianceLevel::Level4,
        passed: result.is_ok(),
        error: result.err().map(|e| e.to_string()),
        category: TestCategory::Security,
        weight: 1.0,
    }
}

async fn level4_missing_token_returns_401(cfg: &TestConfig, client: &HttpClient) -> TestCaseResult {
    let result = async {
        let url = format!(
            "{}/objects/{}",
            cfg.services.drs_url.trim_end_matches('/'),
            auth_test_object_id()
        );
        let resp = client.inner().get(&url).send().await?;
        anyhow::ensure!(
            resp.status() == 401,
            "Missing token must return 401 Unauthorized, got {}",
            resp.status()
        );
        Ok::<(), anyhow::Error>(())
    }
    .await;

    TestCaseResult {
        name: "Auth: missing token returns 401".into(),
        level: ComplianceLevel::Level4,
        passed: result.is_ok(),
        error: result.err().map(|e| e.to_string()),
        category: TestCategory::Security,
        weight: 1.0,
    }
}
