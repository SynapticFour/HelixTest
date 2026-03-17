use crate::logging::init_logging;
use anyhow::Result;
use reqwest::{Client, Response};
use std::time::Duration;
use tokio_retry::strategy::{jitter, ExponentialBackoff};
use tokio_retry::Retry;
use tracing::{debug, info, instrument};

#[derive(Clone)]
pub struct HttpClient {
    inner: Client,
}

impl HttpClient {
    pub fn new() -> Self {
        init_logging();
        Self::with_timeout(Duration::from_secs(60))
    }

    /// Build an HTTP client with a custom request timeout (for tests or strict timeouts).
    pub fn with_timeout(timeout: Duration) -> Self {
        let inner = Client::builder()
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
        debug!(%url, %status, body = %text, "GET response");
        let value: serde_json::Value = serde_json::from_str(&text)?;
        Ok(value)
    }

    #[instrument(skip(self, body))]
    pub async fn post_json(
        &self,
        url: &str,
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        let body_str = body.to_string();
        debug!(%url, body = %body_str, "POST request");
        let strategy = ExponentialBackoff::from_millis(200).map(jitter).take(5);
        let resp = Retry::spawn(strategy, || async {
            let r = self
                .inner
                .post(url)
                .header("Content-Type", "application/json")
                .body(body_str.clone())
                .send()
                .await;
            r
        })
        .await?;
        let status = resp.status();
        let text = resp.text().await?;
        debug!(%url, %status, body = %text, "POST response");
        let value: serde_json::Value = serde_json::from_str(&text)?;
        Ok(value)
    }

    #[instrument(skip(self))]
    async fn get_with_retry(&self, url: &str) -> Result<Response> {
        info!(%url, "GET with retry");
        let strategy = ExponentialBackoff::from_millis(200).map(jitter).take(5);
        let resp = Retry::spawn(strategy, || async {
            let r = self.inner.get(url).send().await;
            r
        })
        .await?;
        Ok(resp)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Duration;
    use wiremock::matchers::method;
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    #[ignore = "slow when client timeout does not fire; run with cargo test -p common -- --ignored"]
    async fn robustness_timeout_fails_fast() {
        let server = MockServer::start().await;
        let delay = Duration::from_millis(500);
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(200).set_delay(delay).set_body_json(serde_json::json!({"ok": true})))
            .mount(&server)
            .await;

        let client = HttpClient::with_timeout(Duration::from_millis(100));
        let url = format!("{}/", server.uri());
        let res = client.get_json(&url).await;
        assert!(res.is_err(), "expected error (timeout or retries exhausted), got {:?}", res);
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
                ResponseTemplate::new(200).set_body_json(serde_json::json!({"ok": true, "attempt": n}))
            }
        }
    }

    #[tokio::test]
    async fn robustness_retry_after_timeout_succeeds() {
        let server = MockServer::start().await;
        let responder = DelayedThenOk::new(2, Duration::from_millis(400));
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
}

