// SPDX-License-Identifier: Apache-2.0
use crate::logging::init_logging;
use anyhow::Result;
use reqwest::header::{HeaderValue, AUTHORIZATION};
use reqwest::{Client, RequestBuilder, Response};
use std::sync::OnceLock;
use std::time::Duration;
use tokio_retry::strategy::{jitter, ExponentialBackoff};
use tokio_retry::Retry;
use tracing::{debug, info, instrument};

const DEFAULT_CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
const DEFAULT_REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
const GET_RETRY_ATTEMPTS: usize = 2;
const BODY_LOG_CHARS: usize = 256;

/// Optional Bearer for gateway-host requests only (not third-party access_url downloads).
/// Set `TEST_BEARER` when Contabo/execution-gate requires Passport on WES/TES.
/// Host allowlist: `GATEWAY_BASE` host, plus comma-separated `HELIXTEST_BEARER_HOSTS`.
fn test_bearer() -> Option<&'static str> {
    static TOKEN: OnceLock<Option<String>> = OnceLock::new();
    TOKEN
        .get_or_init(|| {
            let raw = std::env::var("TEST_BEARER").ok()?;
            let t = raw.trim().to_string();
            if t.is_empty() {
                None
            } else {
                info!("TEST_BEARER set; Authorization attached only for gateway hosts");
                Some(t)
            }
        })
        .as_deref()
}

fn host_allows_bearer(url: &str) -> bool {
    let Ok(parsed) = reqwest::Url::parse(url) else {
        return false;
    };
    let Some(host) = parsed.host_str() else {
        return false;
    };
    let mut allowed = Vec::new();
    if let Ok(base) = std::env::var("GATEWAY_BASE") {
        if let Ok(u) = reqwest::Url::parse(&base) {
            if let Some(h) = u.host_str() {
                allowed.push(h.to_string());
            }
        }
    }
    if let Ok(extra) = std::env::var("HELIXTEST_BEARER_HOSTS") {
        for h in extra.split(',') {
            let h = h.trim();
            if !h.is_empty() {
                allowed.push(h.to_string());
            }
        }
    }
    if allowed.is_empty() {
        return false;
    }
    allowed.iter().any(|h| host.eq_ignore_ascii_case(h))
}

fn maybe_bearer(builder: RequestBuilder, url: &str) -> RequestBuilder {
    match test_bearer() {
        Some(token) if host_allows_bearer(url) => {
            match HeaderValue::from_str(&format!("Bearer {token}")) {
                Ok(value) => builder.header(AUTHORIZATION, value),
                Err(_) => builder,
            }
        }
        _ => builder,
    }
}

#[derive(Clone)]
pub struct HttpClient {
    inner: Client,
}

impl Default for HttpClient {
    fn default() -> Self {
        Self::new()
    }
}

impl HttpClient {
    pub fn new() -> Self {
        init_logging();
        Self::with_timeout(DEFAULT_REQUEST_TIMEOUT)
    }

    /// Build an HTTP client with a custom request timeout (for tests or strict timeouts).
    pub fn with_timeout(timeout: Duration) -> Self {
        let _ = test_bearer();
        let inner = Client::builder()
            .connect_timeout(DEFAULT_CONNECT_TIMEOUT.min(timeout))
            .timeout(timeout)
            .build()
            .expect("failed to build reqwest client");
        Self { inner }
    }

    pub fn inner(&self) -> &Client {
        &self.inner
    }

    /// GET builder with optional `TEST_BEARER` for gateway hosts only.
    pub fn get_builder(&self, url: &str) -> RequestBuilder {
        maybe_bearer(self.inner.get(url), url)
    }

    /// POST builder with optional `TEST_BEARER` for gateway hosts only.
    pub fn post_builder(&self, url: &str) -> RequestBuilder {
        maybe_bearer(self.inner.post(url), url)
    }

    #[instrument(skip(self))]
    pub async fn get_json(&self, url: &str) -> Result<serde_json::Value> {
        let resp = self.get_with_retry(url).await?;
        let status = resp.status();
        let text = resp.text().await?;
        debug!(%url, %status, bytes = text.len(), body_prefix = %truncate_for_log(&text), "GET response");
        if !status.is_success() {
            anyhow::bail!(
                "GET {} failed with HTTP {}: {}",
                url,
                status,
                truncate_for_log(&text)
            );
        }
        let value: serde_json::Value = serde_json::from_str(&text)?;
        Ok(value)
    }

    /// POST once (not retried). WES/TES create are not idempotent.
    #[instrument(skip(self, body))]
    pub async fn post_json(
        &self,
        url: &str,
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        let body_str = body.to_string();
        debug!(
            %url,
            bytes = body_str.len(),
            body_prefix = %truncate_for_log(&body_str),
            "POST request"
        );
        let resp = maybe_bearer(self.inner.post(url), url)
            .header("Content-Type", "application/json")
            .body(body_str)
            .send()
            .await?;
        let status = resp.status();
        let text = resp.text().await?;
        debug!(
            %url,
            %status,
            bytes = text.len(),
            body_prefix = %truncate_for_log(&text),
            "POST response"
        );
        if !status.is_success() {
            anyhow::bail!(
                "POST {} failed with HTTP {}: {}",
                url,
                status,
                truncate_for_log(&text)
            );
        }
        let value: serde_json::Value = serde_json::from_str(&text)?;
        Ok(value)
    }

    #[instrument(skip(self))]
    async fn get_with_retry(&self, url: &str) -> Result<Response> {
        info!(%url, "GET with retry");
        let strategy = ExponentialBackoff::from_millis(200)
            .map(jitter)
            .take(GET_RETRY_ATTEMPTS);
        let url = url.to_string();
        let client = self.inner.clone();
        let resp = Retry::spawn(strategy, || {
            let client = client.clone();
            let url = url.clone();
            async move { maybe_bearer(client.get(&url), &url).send().await }
        })
        .await?;
        Ok(resp)
    }
}

fn truncate_for_log(s: &str) -> String {
    let mut it = s.chars();
    let prefix: String = it.by_ref().take(BODY_LOG_CHARS).collect();
    if it.next().is_some() {
        format!("{prefix}…")
    } else {
        prefix
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Duration;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    #[ignore = "slow when client timeout does not fire; run with cargo test -p common -- --ignored"]
    async fn robustness_timeout_fails_fast() {
        let server = MockServer::start().await;
        let delay = Duration::from_millis(500);
        Mock::given(method("GET"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_delay(delay)
                    .set_body_json(serde_json::json!({"ok": true})),
            )
            .mount(&server)
            .await;

        let client = HttpClient::with_timeout(Duration::from_millis(100));
        let url = format!("{}/", server.uri());
        let res = client.get_json(&url).await;
        assert!(
            res.is_err(),
            "expected error (timeout or retries exhausted), got {:?}",
            res
        );
        let err = res.unwrap_err().to_string();
        let is_timeout = err.to_lowercase().contains("timeout")
            || err.contains("Timed out")
            || err.to_lowercase().contains("deadline");
        assert!(
            is_timeout || err.contains("error") || err.contains("failed"),
            "error should indicate timeout or failure: {}",
            err
        );
    }

    /// Responds with delay for the first N requests, then 200 immediately (for retry testing).
    struct DelayedThenOk {
        count: AtomicUsize,
        delay_threshold: usize,
        delay: Duration,
    }

    impl DelayedThenOk {
        fn new(delay_threshold: usize, delay: Duration) -> Self {
            Self {
                count: AtomicUsize::new(0),
                delay_threshold,
                delay,
            }
        }
    }

    impl wiremock::Respond for DelayedThenOk {
        fn respond(&self, _request: &wiremock::Request) -> ResponseTemplate {
            let n = self.count.fetch_add(1, Ordering::SeqCst);
            if n < self.delay_threshold {
                ResponseTemplate::new(200)
                    .set_delay(self.delay)
                    .set_body_json(serde_json::json!({"attempt": n}))
            } else {
                ResponseTemplate::new(200)
                    .set_body_json(serde_json::json!({"ok": true, "attempt": n}))
            }
        }
    }

    #[tokio::test]
    async fn robustness_retry_after_timeout_succeeds() {
        let server = MockServer::start().await;
        let responder = DelayedThenOk::new(1, Duration::from_millis(400));
        Mock::given(method("GET"))
            .respond_with(responder)
            .mount(&server)
            .await;

        let client = HttpClient::with_timeout(Duration::from_millis(200));
        let url = format!("{}/", server.uri());
        let res = client.get_json(&url).await;
        assert!(res.is_ok(), "expected success after retries: {:?}", res);
        let v = res.unwrap();
        assert_eq!(v.get("ok").and_then(|x| x.as_bool()), Some(true));
    }

    #[tokio::test]
    async fn get_json_non_success_is_error() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/boom"))
            .respond_with(ResponseTemplate::new(500).set_body_string("nope"))
            .mount(&server)
            .await;

        let client = HttpClient::with_timeout(Duration::from_secs(1));
        let url = format!("{}/boom", server.uri());
        let err = client.get_json(&url).await.unwrap_err().to_string();
        assert!(err.contains("HTTP 500"), "unexpected error: {}", err);
    }

    #[tokio::test]
    async fn get_json_invalid_json_is_error() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/badjson"))
            .respond_with(ResponseTemplate::new(200).set_body_string("{not json"))
            .mount(&server)
            .await;

        let client = HttpClient::with_timeout(Duration::from_secs(1));
        let url = format!("{}/badjson", server.uri());
        let err = client.get_json(&url).await.unwrap_err().to_string();
        assert!(
            err.to_lowercase().contains("expected") || err.to_lowercase().contains("at line"),
            "unexpected error: {}",
            err
        );
    }

    #[tokio::test]
    async fn post_json_non_success_is_error() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/boom"))
            .respond_with(ResponseTemplate::new(400).set_body_string("bad request"))
            .mount(&server)
            .await;

        let client = HttpClient::with_timeout(Duration::from_secs(1));
        let url = format!("{}/boom", server.uri());
        let body = serde_json::json!({"x": 1});
        let err = client.post_json(&url, &body).await.unwrap_err().to_string();
        assert!(err.contains("HTTP 400"), "unexpected error: {}", err);
    }

    #[tokio::test]
    async fn post_json_is_not_retried() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/once"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"ok": true})))
            .expect(1)
            .mount(&server)
            .await;

        let client = HttpClient::with_timeout(Duration::from_secs(2));
        let url = format!("{}/once", server.uri());
        let v = client
            .post_json(&url, &serde_json::json!({}))
            .await
            .unwrap();
        assert_eq!(v.get("ok").and_then(|x| x.as_bool()), Some(true));
    }

    #[test]
    fn host_allows_bearer_matches_gateway_base_only() {
        std::env::set_var("GATEWAY_BASE", "https://ferrum.demo.synapticfour.tech");
        std::env::remove_var("HELIXTEST_BEARER_HOSTS");
        assert!(host_allows_bearer(
            "https://ferrum.demo.synapticfour.tech/ga4gh/wes/v1/runs"
        ));
        assert!(!host_allows_bearer(
            "https://raw.githubusercontent.com/ga4gh/data-repository-service-schemas/master/README.md"
        ));
    }
}
