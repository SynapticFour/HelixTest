use anyhow::Result;
use chrono::Duration;
use common::auth::build_jwt;
use common::config::TestConfig;
use common::http::HttpClient;
use serde_json::json;

fn auth_secret() -> String {
    std::env::var("HELIXTEST_SHARED_SECRET").unwrap_or_else(|_| "test-secret".to_owned())
}

#[tokio::test]
async fn valid_token_grants_access() -> Result<()> {
    let cfg = TestConfig::from_env_or_file()?;
    let client = HttpClient::new();
    let token = build_jwt(
        "https://auth.test",
        "test-user",
        "drs",
        "drs.read",
        Duration::minutes(5),
        &auth_secret(),
    )?;
    let url = format!(
        "{}/objects/{}",
        cfg.services.drs_url.trim_end_matches('/'),
        "test-object-1"
    );
    let resp = client
        .inner()
        .get(&url)
        .bearer_auth(&token)
        .send()
        .await?;
    assert_eq!(resp.status(), 200, "Valid token must be accepted");
    Ok(())
}

#[tokio::test]
async fn expired_token_is_rejected() -> Result<()> {
    let cfg = TestConfig::from_env_or_file()?;
    let client = HttpClient::new();
    let token = build_jwt(
        "https://auth.test",
        "test-user",
        "drs",
        "drs.read",
        Duration::minutes(-5),
        &auth_secret(),
    )?;
    let url = format!(
        "{}/objects/{}",
        cfg.services.drs_url.trim_end_matches('/'),
        "test-object-1"
    );
    let resp = client
        .inner()
        .get(&url)
        .bearer_auth(&token)
        .send()
        .await?;
    assert!(
        resp.status().is_client_error(),
        "Expired token must be rejected with 4xx, got {}",
        resp.status()
    );
    Ok(())
}

#[tokio::test]
async fn missing_scope_denies_access() -> Result<()> {
    let cfg = TestConfig::from_env_or_file()?;
    let client = HttpClient::new();
    let token = build_jwt(
        "https://auth.test",
        "test-user",
        "drs",
        "wes.run", // wrong scope
        Duration::minutes(5),
        &auth_secret(),
    )?;
    let url = format!(
        "{}/objects/{}",
        cfg.services.drs_url.trim_end_matches('/'),
        "test-object-1"
    );
    let resp = client
        .inner()
        .get(&url)
        .bearer_auth(&token)
        .send()
        .await?;
    assert!(
        resp.status() == 403 || resp.status() == 401,
        "Missing scope must deny access, got {}",
        resp.status()
    );
    Ok(())
}

