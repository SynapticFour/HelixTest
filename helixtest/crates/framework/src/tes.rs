use anyhow::Result;
use common::config::TestConfig;
use common::ga4gh_schemas;
use common::http::HttpClient;
use common::report::{ComplianceLevel, ServiceKind, ServiceReport, TestCaseResult, TestCategory};
use common::util::sha256_file;
use serde_json::json;
use std::path::Path;
use std::time::Duration;
use tokio::time::sleep;
use tracing::info;

use crate::{Features, Mode};

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
    let res = client.inner().get(&url).send().await;
    match res {
        Ok(resp) => TestCaseResult {
            name: "TES /tasks reachable".into(),
            level: ComplianceLevel::Level0,
            passed: resp.status().is_success() || resp.status().is_client_error(),
            error: (!resp.status().is_success() && !resp.status().is_client_error())
                .then(|| format!("Unexpected HTTP status: {}", resp.status())),
            category: TestCategory::Other,
            weight: 1.0,
        },
        Err(e) => TestCaseResult {
            name: "TES /tasks reachable".into(),
            level: ComplianceLevel::Level0,
            passed: false,
            error: Some(e.to_string()),
            category: TestCategory::Other,
            weight: 1.0,
        },
    }
}

async fn level1_task_schema(cfg: &TestConfig, client: &HttpClient) -> TestCaseResult {
    let res = async {
        // Submit a minimal TES task; exact schema may vary by implementation.
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

    TestCaseResult {
        name: "TES task schema (create + status)".into(),
        level: ComplianceLevel::Level1,
        passed: res.is_ok(),
        error: res.err().map(|e| e.to_string()),
        category: TestCategory::Schema,
        weight: 1.0,
    }
}

async fn level2_task_lifecycle_and_checksum(
    cfg: &TestConfig,
    client: &HttpClient,
) -> TestCaseResult {
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

        // Poll task until terminal
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
                break state.to_owned();
            }
            if start.elapsed() > timeout {
                anyhow::bail!("Timed out waiting for TES task {}", task_id);
            }
            sleep(Duration::from_secs(2)).await;
        };
        if final_state != "COMPLETE" {
            anyhow::bail!("Expected TES task to COMPLETE, got {}", final_state);
        }

        // Checksum validation for TES output file under test-data
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("test-data");
        let expected_checksum_path = root
            .join("expected")
            .join("workflows")
            .join("tes_echo_out.txt.sha256");
        let expected_checksum =
            std::fs::read_to_string(&expected_checksum_path)?.trim().to_owned();

        let produced_file = root
            .join("workflows")
            .join("outputs")
            .join("tes_echo_out.txt");
        let actual_checksum = sha256_file(&produced_file)?;
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

    TestCaseResult {
        name: "TES task lifecycle + checksum".into(),
        level: ComplianceLevel::Level2,
        passed: res.is_ok(),
        error: res.err().map(|e| e.to_string()),
        category: TestCategory::Checksum,
        weight: 1.0,
    }
}
