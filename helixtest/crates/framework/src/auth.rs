// SPDX-License-Identifier: Apache-2.0
//! Level 4 security: HMAC-SHA256 JWT fixture against DRS (shared secret), plus optional
//! token-protected endpoints. This is **not** GA4GH Passports/OIDC; Passport checks live in
//! `--mode ferrum+infra` (`infra.rs`).

use crate::{level0_http, Features, Mode};
use anyhow::Result;
use chrono::Duration;
use common::auth::build_jwt;
use common::config::TestConfig;
use common::http::HttpClient;
use common::report::{ComplianceLevel, ServiceKind, ServiceReport, TestCaseResult, TestCategory};

fn hmac_secret() -> Option<String> {
    std::env::var("HELIXTEST_SHARED_SECRET")
        .ok()
        .filter(|s| !s.is_empty())
}

fn skip_hmac_fixture(name: &str) -> TestCaseResult {
    TestCaseResult::skip(
        name,
        ComplianceLevel::Level4,
        TestCategory::Security,
        "HELIXTEST_SHARED_SECRET unset; HMAC JWT fixture not run (not Passports)",
    )
}

fn auth_test_object_id() -> String {
    std::env::var("HELIXTEST_AUTH_OBJECT_ID").unwrap_or_else(|_| "test-object-1".to_owned())
}

fn token_only_mode(cfg: &TestConfig) -> bool {
    cfg.auth_checks
        .mode
        .as_deref()
        .map(|m| m.eq_ignore_ascii_case("token-protected-endpoints"))
        .unwrap_or(false)
}

pub async fn run_auth_checks(
    _mode: Mode,
    _features: &Features,
    cfg: &TestConfig,
    client: &HttpClient,
) -> Result<ServiceReport> {
    if token_only_mode(cfg) {
        let mut tests = vec![level0_auth_url(cfg, client).await];
        tests.extend(run_token_protected_endpoint_checks(cfg, client).await);
        return Ok(ServiceReport {
            service: ServiceKind::Auth,
            tests,
        });
    }
    let mut tests = Vec::new();
    tests.push(level0_auth_url(cfg, client).await);
    tests.push(level4_valid_token_grants_access(cfg, client).await);
    tests.push(level4_expired_token_rejected(cfg, client).await);
    tests.push(level4_wrong_scope_denied(cfg, client).await);
    tests.push(level4_missing_token_returns_401(cfg, client).await);

    Ok(ServiceReport {
        service: ServiceKind::Auth,
        tests,
    })
}

async fn level0_auth_url(cfg: &TestConfig, client: &HttpClient) -> TestCaseResult {
    let base = cfg.services.auth_url.trim_end_matches('/');
    if base.is_empty() {
        return TestCaseResult::skip(
            "Auth service URL reachable",
            ComplianceLevel::Level0,
            TestCategory::Other,
            "auth_url is empty",
        );
    }
    let url = format!("{}/service-info", base);
    level0_http(
        "Auth /service-info reachable (auth_url)",
        client.inner().get(&url).send().await,
    )
}

async fn level4_valid_token_grants_access(cfg: &TestConfig, client: &HttpClient) -> TestCaseResult {
    const NAME: &str = "Auth (HMAC JWT fixture): valid token grants DRS access";
    let Some(secret) = hmac_secret() else {
        return skip_hmac_fixture(NAME);
    };
    let result = async {
        let token = build_jwt(
            "https://auth.ga4gh.test",
            "test-user",
            "drs",
            "drs.read",
            Duration::minutes(5),
            &secret,
        )?;
        let url = format!(
            "{}/objects/{}",
            cfg.services.drs_url.trim_end_matches('/'),
            auth_test_object_id()
        );
        let resp = client.inner().get(&url).bearer_auth(&token).send().await?;
        anyhow::ensure!(
            resp.status().is_success(),
            "Valid HMAC JWT should be accepted, got {}",
            resp.status()
        );
        Ok::<(), anyhow::Error>(())
    }
    .await;

    TestCaseResult::from_outcome(
        NAME,
        ComplianceLevel::Level4,
        TestCategory::Security,
        result,
    )
}

async fn run_token_protected_endpoint_checks(
    cfg: &TestConfig,
    client: &HttpClient,
) -> Vec<TestCaseResult> {
    if cfg.auth_checks.protected_endpoints.is_empty() {
        return vec![TestCaseResult::skip(
            "Auth token-only mode configured but no protected endpoints set",
            ComplianceLevel::Level4,
            TestCategory::Security,
            "set [auth_checks].protected_endpoints in profile/config",
        )];
    }
    let token_env = cfg
        .auth_checks
        .valid_token_env
        .clone()
        .unwrap_or_else(|| "TEST_BEARER".to_string());
    let valid_token = std::env::var(&token_env)
        .ok()
        .filter(|s| !s.trim().is_empty());
    let invalid_token = cfg
        .auth_checks
        .invalid_token
        .clone()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| "helixtest-invalid-token".to_string());

    let mut tests = Vec::new();
    for endpoint in &cfg.auth_checks.protected_endpoints {
        let method = endpoint
            .method
            .as_deref()
            .unwrap_or("GET")
            .to_ascii_uppercase();
        let req = |bearer: Option<&str>| {
            let builder = match method.as_str() {
                "POST" => client.inner().post(&endpoint.url),
                "PUT" => client.inner().put(&endpoint.url),
                "DELETE" => client.inner().delete(&endpoint.url),
                _ => client.inner().get(&endpoint.url),
            };
            match bearer {
                Some(token) => builder.bearer_auth(token),
                None => builder,
            }
        };

        let no_token = req(None).send().await;
        tests.push(match no_token {
            Ok(resp) if resp.status().as_u16() == 401 => TestCaseResult::pass(
                format!("Auth token-only: {} without bearer -> 401", endpoint.name),
                ComplianceLevel::Level4,
                TestCategory::Security,
            ),
            Ok(resp) => TestCaseResult::fail(
                format!("Auth token-only: {} without bearer -> 401", endpoint.name),
                ComplianceLevel::Level4,
                TestCategory::Security,
                format!("expected 401, got {}", resp.status()),
            ),
            Err(e) => TestCaseResult::fail(
                format!("Auth token-only: {} without bearer -> 401", endpoint.name),
                ComplianceLevel::Level4,
                TestCategory::Security,
                e,
            ),
        });

        if endpoint.check_invalid_token.unwrap_or(true) {
            let invalid = req(Some(&invalid_token)).send().await;
            tests.push(match invalid {
                Ok(resp) if resp.status().as_u16() == 401 => TestCaseResult::pass(
                    format!("Auth token-only: {} invalid bearer -> 401", endpoint.name),
                    ComplianceLevel::Level4,
                    TestCategory::Security,
                ),
                Ok(resp) => TestCaseResult::fail(
                    format!("Auth token-only: {} invalid bearer -> 401", endpoint.name),
                    ComplianceLevel::Level4,
                    TestCategory::Security,
                    format!("expected 401, got {}", resp.status()),
                ),
                Err(e) => TestCaseResult::fail(
                    format!("Auth token-only: {} invalid bearer -> 401", endpoint.name),
                    ComplianceLevel::Level4,
                    TestCategory::Security,
                    e,
                ),
            });
        }

        let valid_name = format!("Auth token-only: {} valid bearer -> 2xx", endpoint.name);
        let Some(token) = valid_token.as_deref() else {
            tests.push(TestCaseResult::skip(
                valid_name,
                ComplianceLevel::Level4,
                TestCategory::Security,
                format!("set {} env var", token_env),
            ));
            continue;
        };
        let valid = req(Some(token)).send().await;
        tests.push(match valid {
            Ok(resp) if resp.status().is_success() => {
                TestCaseResult::pass(valid_name, ComplianceLevel::Level4, TestCategory::Security)
            }
            Ok(resp) => TestCaseResult::fail(
                valid_name,
                ComplianceLevel::Level4,
                TestCategory::Security,
                format!("expected 2xx, got {}", resp.status()),
            ),
            Err(e) => TestCaseResult::fail(
                valid_name,
                ComplianceLevel::Level4,
                TestCategory::Security,
                e,
            ),
        });
    }
    tests
}

async fn level4_expired_token_rejected(cfg: &TestConfig, client: &HttpClient) -> TestCaseResult {
    const NAME: &str = "Auth (HMAC JWT fixture): expired token rejected";
    let Some(secret) = hmac_secret() else {
        return skip_hmac_fixture(NAME);
    };
    let result = async {
        let token = build_jwt(
            "https://auth.ga4gh.test",
            "test-user",
            "drs",
            "drs.read",
            Duration::minutes(-5),
            &secret,
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

    TestCaseResult::from_outcome(
        NAME,
        ComplianceLevel::Level4,
        TestCategory::Security,
        result,
    )
}

async fn level4_wrong_scope_denied(cfg: &TestConfig, client: &HttpClient) -> TestCaseResult {
    const NAME: &str = "Auth (HMAC JWT fixture): wrong scope denied";
    let Some(secret) = hmac_secret() else {
        return skip_hmac_fixture(NAME);
    };
    let result = async {
        let token = build_jwt(
            "https://auth.ga4gh.test",
            "test-user",
            "drs",
            "wes.run",
            Duration::minutes(5),
            &secret,
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

    TestCaseResult::from_outcome(
        NAME,
        ComplianceLevel::Level4,
        TestCategory::Security,
        result,
    )
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

    TestCaseResult::from_outcome(
        "Auth (HMAC JWT fixture): missing token returns 401",
        ComplianceLevel::Level4,
        TestCategory::Security,
        result,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use common::config::AuthChecksConfig;

    #[test]
    fn token_only_mode_detection_works() {
        let cfg = TestConfig {
            services: common::config::ServiceConfig {
                wes_url: String::new(),
                tes_url: String::new(),
                drs_url: String::new(),
                trs_url: String::new(),
                beacon_url: String::new(),
                auth_url: String::new(),
                htsget_url: None,
            },
            subset: common::config::SubsetConfig::default(),
            auth_checks: AuthChecksConfig {
                mode: Some("token-protected-endpoints".into()),
                ..Default::default()
            },
        };
        assert!(token_only_mode(&cfg));
    }

    #[tokio::test]
    async fn token_only_mode_without_endpoints_yields_skip() {
        let cfg = TestConfig {
            services: common::config::ServiceConfig {
                wes_url: String::new(),
                tes_url: String::new(),
                drs_url: String::new(),
                trs_url: String::new(),
                beacon_url: String::new(),
                auth_url: String::new(),
                htsget_url: None,
            },
            subset: common::config::SubsetConfig::default(),
            auth_checks: AuthChecksConfig {
                mode: Some("token-protected-endpoints".into()),
                protected_endpoints: Vec::new(),
                valid_token_env: None,
                invalid_token: None,
            },
        };
        let tests = run_token_protected_endpoint_checks(&cfg, &HttpClient::new()).await;
        assert_eq!(tests.len(), 1);
        assert_eq!(tests[0].status, common::report::TestStatus::Skip);
        assert!(tests[0].error.as_deref().unwrap_or("").contains("skipped"));
    }
}
