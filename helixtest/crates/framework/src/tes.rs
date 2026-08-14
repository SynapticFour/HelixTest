use anyhow::{Context, Result};
use common::config::TestConfig;
use common::ga4gh_schemas;
use common::http::HttpClient;
use common::report::{ComplianceLevel, ServiceKind, ServiceReport, TestCaseResult, TestCategory};
use common::util::{sha256_bytes, sha256_file_if_fresh, test_data_dir};
use serde_json::json;
use std::time::{Duration, SystemTime};
use tokio::time::sleep;
use tracing::info;

use crate::{level0_http, Features, Mode};

pub async fn run_tes_checks(
    _mode: Mode,
    _features: &Features,
    cfg: &TestConfig,
    client: &HttpClient,
) -> Result<ServiceReport> {
    let mut tests = Vec::new();
    tests.push(level0_reachable(cfg, client).await);
    tests.push(level1_task_schema(cfg, client).await);
    tests.push(level2_task_lifecycle_and_checksum(cfg, client).await);

    Ok(ServiceReport {
        service: ServiceKind::Tes,
        tests,
    })
}

async fn level0_reachable(cfg: &TestConfig, client: &HttpClient) -> TestCaseResult {
    let url = format!("{}/tasks", cfg.services.tes_url.trim_end_matches('/'));
    level0_http(
        "TES /tasks reachable",
        client.inner().get(&url).send().await,
    )
}

async fn level1_task_schema(cfg: &TestConfig, client: &HttpClient) -> TestCaseResult {
    let res = async {
        let url = format!("{}/tasks", cfg.services.tes_url.trim_end_matches('/'));
        let body = json!({
            "name": "helix-test-echo",
            "executors": [{
                "image": "alpine",
                "command": ["echo", "hello-tes"]
            }]
        });
        let resp = client.inner().post(&url).json(&body).send().await?;
        if !resp.status().is_success() {
            anyhow::bail!("TES task submission failed: {}", resp.status());
        }
        let v: serde_json::Value = resp.json().await?;
        ga4gh_schemas::validate_tes_create_task_response(&v)?;

        let task_id = v
            .get("id")
            .and_then(|x| x.as_str())
            .ok_or_else(|| anyhow::anyhow!("TES createTask response missing id: {}", v))?;

        let status_url = format!(
            "{}/tasks/{}",
            cfg.services.tes_url.trim_end_matches('/'),
            task_id
        );
        let status_val = client.get_json(&status_url).await?;
        ga4gh_schemas::validate_tes_task(&status_val)?;

        Ok::<(), anyhow::Error>(())
    }
    .await;

    TestCaseResult::from_outcome(
        "TES task schema (create + status)",
        ComplianceLevel::Level1,
        TestCategory::Schema,
        res,
    )
}

async fn download_sha256(client: &HttpClient, url: &str) -> Result<String> {
    let resp = client.inner().get(url).send().await?;
    if !resp.status().is_success() {
        anyhow::bail!("TES output download failed: {}", resp.status());
    }
    let bytes = resp.bytes().await?;
    Ok(sha256_bytes(&bytes))
}

fn tes_output_urls(task: &serde_json::Value) -> Vec<String> {
    let mut urls = Vec::new();
    if let Some(outputs) = task.get("outputs").and_then(|o| o.as_array()) {
        for o in outputs {
            if let Some(u) = o.get("url").and_then(|x| x.as_str()) {
                urls.push(u.to_string());
            }
        }
    }
    urls
}

async fn level2_task_lifecycle_and_checksum(
    cfg: &TestConfig,
    client: &HttpClient,
) -> TestCaseResult {
    let submitted_at = SystemTime::now();
    let res = async {
        let url = format!("{}/tasks", cfg.services.tes_url.trim_end_matches('/'));
        let body = json!({
            "name": "helix-test-echo-checksum",
            "executors": [{
                "image": "alpine",
                "command": ["sh", "-c", "echo hello-tes > /test-data/workflows/outputs/tes_echo_out.txt"]
            }]
        });
        let resp = client.inner().post(&url).json(&body).send().await?;
        if !resp.status().is_success() {
            anyhow::bail!("TES task submission failed: {}", resp.status());
        }
        let v: serde_json::Value = resp.json().await?;
        let task_id = v
            .get("id")
            .and_then(|x| x.as_str())
            .ok_or_else(|| anyhow::anyhow!("TES createTask response missing id: {}", v))?;

        info!(%task_id, "Submitted TES task for lifecycle + checksum test");

        let status_url = format!(
            "{}/tasks/{}",
            cfg.services.tes_url.trim_end_matches('/'),
            task_id
        );
        let start = std::time::Instant::now();
        let timeout = Duration::from_secs(300);
        let final_state = loop {
            let v = client.get_json(&status_url).await?;
            let state = v
                .get("state")
                .and_then(|x| x.as_str())
                .ok_or_else(|| anyhow::anyhow!("TES task status missing state: {}", v))?;
            info!(%task_id, %state, "TES task state");
            if matches!(
                state,
                "COMPLETE" | "EXECUTOR_ERROR" | "SYSTEM_ERROR" | "CANCELED"
            ) {
                break (state.to_owned(), v);
            }
            if start.elapsed() > timeout {
                anyhow::bail!("Timed out waiting for TES task {}", task_id);
            }
            sleep(Duration::from_secs(2)).await;
        };
        if final_state.0 != "COMPLETE" {
            anyhow::bail!("Expected TES task to COMPLETE, got {}", final_state.0);
        }
        let task_json = final_state.1;

        let expected_checksum_path = test_data_dir()?
            .join("expected")
            .join("workflows")
            .join("tes_echo_out.txt.sha256");
        let expected_checksum = std::fs::read_to_string(&expected_checksum_path)
            .with_context(|| {
                format!(
                    "missing golden checksum {}",
                    expected_checksum_path.display()
                )
            })?
            .trim()
            .to_owned();

        let mut actual_checksum = None;
        for out_url in tes_output_urls(&task_json) {
            match download_sha256(client, &out_url).await {
                Ok(h) => {
                    actual_checksum = Some(h);
                    break;
                }
                Err(e) => info!(url = %out_url, error = %e, "TES output URL not downloadable"),
            }
        }

        if actual_checksum.is_none() {
            let produced_file = test_data_dir()?
                .join("workflows")
                .join("outputs")
                .join("tes_echo_out.txt");
            match sha256_file_if_fresh(&produced_file, submitted_at) {
                Ok(h) => actual_checksum = Some(h),
                Err(e) => info!(error = %e, "TES local output not usable"),
            }
        }

        let actual_checksum = actual_checksum.ok_or_else(|| {
            anyhow::anyhow!(
                "TES COMPLETE but no output bytes: task JSON has no downloadable outputs[].url and no fresh local file under test-data/workflows/outputs/tes_echo_out.txt"
            )
        })?;

        info!(
            %task_id,
            expected = %expected_checksum,
            actual = %actual_checksum,
            "TES checksum comparison"
        );
        if !actual_checksum.eq_ignore_ascii_case(&expected_checksum) {
            anyhow::bail!(
                "TES output checksum mismatch: expected {}, got {}",
                expected_checksum,
                actual_checksum
            );
        }

        Ok::<(), anyhow::Error>(())
    }
    .await;

    TestCaseResult::from_outcome(
        "TES task lifecycle + checksum (non-terminal states allowed until terminal)",
        ComplianceLevel::Level2,
        TestCategory::Checksum,
        res,
    )
}
