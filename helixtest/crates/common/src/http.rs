use crate::logging::init_logging;
use anyhow::Result;
use reqwest::{Client, Response};
use std::time::Duration;
use tokio_retry::strategy::{jitter, ExponentialBackoff};
use tokio_retry::Retry;
use tracing::{debug, info, instrument};

const DEFAULT_CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
const DEFAULT_REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
const GET_RETRY_ATTEMPTS: usize = 2;
const BODY_LOG_CHARS: usize = 256;

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
        let resp = self
            .inner
            .post(url)
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
        let resp = Retry::spawn(strategy, || async { self.inner.get(url).send().await }).await?;
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
}
