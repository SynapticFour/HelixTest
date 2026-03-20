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
        let tests = run_token_protected_endpoint_checks(cfg, client).await;
        return Ok(ServiceReport {
            service: ServiceKind::Auth,
            tests,
        });
    }
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

async fn run_token_protected_endpoint_checks(
    cfg: &TestConfig,
    client: &HttpClient,
) -> Vec<TestCaseResult> {
    if cfg.auth_checks.protected_endpoints.is_empty() {
        return vec![TestCaseResult {
            name: "Auth token-only mode configured but no protected endpoints set".into(),
            level: ComplianceLevel::Level4,
            passed: true,
            error: Some("skipped: set [auth_checks].protected_endpoints in profile/config".into()),
            category: TestCategory::Security,
            weight: 1.0,
        }];
    }
    let token_env = cfg
        .auth_checks
        .valid_token_env
        .clone()
        .unwrap_or_else(|| "TEST_BEARER".to_string());
    let valid_token = std::env::var(&token_env).ok().filter(|s| !s.trim().is_empty());
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
        let no_token_passed = no_token
            .as_ref()
            .map(|r| r.status().as_u16() == 401)
            .unwrap_or(false);
        tests.push(TestCaseResult {
            name: format!("Auth token-only: {} without bearer -> 401", endpoint.name),
            level: ComplianceLevel::Level4,
            passed: no_token_passed,
            error: if no_token_passed {
                None
            } else {
                Some(match no_token {
                    Ok(resp) => format!("expected 401, got {}", resp.status()),
                    Err(e) => e.to_string(),
                })
            },
            category: TestCategory::Security,
            weight: 1.0,
        });

        if endpoint.check_invalid_token.unwrap_or(true) {
            let invalid = req(Some(&invalid_token)).send().await;
            let invalid_passed = invalid
                .as_ref()
                .map(|r| r.status().as_u16() == 401)
                .unwrap_or(false);
            tests.push(TestCaseResult {
                name: format!("Auth token-only: {} invalid bearer -> 401", endpoint.name),
                level: ComplianceLevel::Level4,
                passed: invalid_passed,
                error: if invalid_passed {
                    None
                } else {
                    Some(match invalid {
                        Ok(resp) => format!("expected 401, got {}", resp.status()),
                        Err(e) => e.to_string(),
                    })
                },
                category: TestCategory::Security,
                weight: 1.0,
            });
        }

        let valid_name = format!("Auth token-only: {} valid bearer -> 2xx", endpoint.name);
        let Some(token) = valid_token.as_deref() else {
            tests.push(TestCaseResult {
                name: valid_name,
                level: ComplianceLevel::Level4,
                passed: true,
                error: Some(format!("skipped: set {} env var", token_env)),
                category: TestCategory::Security,
                weight: 1.0,
            });
            continue;
        };
        let valid = req(Some(token)).send().await;
        let valid_passed = valid
            .as_ref()
            .map(|r| r.status().is_success())
            .unwrap_or(false);
        tests.push(TestCaseResult {
            name: valid_name,
            level: ComplianceLevel::Level4,
            passed: valid_passed,
            error: if valid_passed {
                None
            } else {
                Some(match valid {
                    Ok(resp) => format!("expected 2xx, got {}", resp.status()),
                    Err(e) => e.to_string(),
                })
            },
            category: TestCategory::Security,
            weight: 1.0,
        });
    }
    tests
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
    async fn token_only_mode_without_endpoints_yields_skip_like_result() {
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
        assert!(tests[0].passed);
        assert!(tests[0].error.as_deref().unwrap_or("").contains("skipped"));
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
