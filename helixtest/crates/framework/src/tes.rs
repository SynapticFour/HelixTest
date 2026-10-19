// SPDX-License-Identifier: Apache-2.0
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

use crate::allowlist::AllowlistExec;
use crate::{level0_http, Features, Mode};

pub async fn run_tes_checks(
    _mode: Mode,
    features: &Features,
    cfg: &TestConfig,
    client: &HttpClient,
) -> Result<ServiceReport> {
    let mut tests = Vec::new();
    tests.push(level0_reachable(cfg, client).await);

    if features.execution_allowlist {
        let al = match AllowlistExec::from_env() {
            Ok(a) => a,
            Err(e) => {
                let msg = e.to_string();
                tests.push(TestCaseResult::fail(
                    "TES allowlist env (fail closed)",
                    ComplianceLevel::Level1,
                    TestCategory::Schema,
                    msg.clone(),
                ));
                tests.push(TestCaseResult::fail(
                    "TES allowlist task lifecycle",
                    ComplianceLevel::Level2,
                    TestCategory::Lifecycle,
                    msg,
                ));
                return Ok(ServiceReport {
                    service: ServiceKind::Tes,
                    tests,
                });
            }
        };
        tests.push(level1_task_schema_allowlist(cfg, client, &al).await);
        tests.push(level2_task_lifecycle_allowlist(cfg, client, &al).await);
    } else {
        tests.push(level1_task_schema(cfg, client).await);
        tests.push(level2_task_lifecycle_and_checksum(cfg, client).await);
    }

    Ok(ServiceReport {
        service: ServiceKind::Tes,
        tests,
    })
}

async fn level0_reachable(cfg: &TestConfig, client: &HttpClient) -> TestCaseResult {
    let url = format!("{}/tasks", cfg.services.tes_url.trim_end_matches('/'));
    level0_http(
        "TES /tasks reachable",
        client.get_builder(&url).send().await,
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
        let resp = client.post_builder(&url).json(&body).send().await?;
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

async fn level1_task_schema_allowlist(
    cfg: &TestConfig,
    client: &HttpClient,
    al: &AllowlistExec,
) -> TestCaseResult {
    let res = async {
        let url = format!("{}/tasks", cfg.services.tes_url.trim_end_matches('/'));
        let body = al.tes_create_body();
        let (status, text) = client.post_raw(&url, &body).await?;
        if !status.is_success() {
            anyhow::bail!("TES allowlist task submission failed: HTTP {status}: {text}");
        }
        let v: serde_json::Value = serde_json::from_str(&text)?;
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
        // Execution-gate returns a slim TES task (id/state/name/outputs). Full GA4GH
        // tesTask (executors, rich logs) is Ferrum-native — do not pretend the gate is Ferrum.
        let sid = status_val.get("id").and_then(|x| x.as_str());
        let state = status_val.get("state").and_then(|x| x.as_str());
        if sid != Some(task_id) {
            anyhow::bail!(
                "TES status id mismatch: expected {task_id}, got {}",
                status_val
            );
        }
        if state.is_none() {
            anyhow::bail!("TES status missing state: {}", status_val);
        }

        Ok::<(), anyhow::Error>(())
    }
    .await;

    TestCaseResult::from_outcome(
        "TES allowlist task schema (create + status id/state)",
        ComplianceLevel::Level1,
        TestCategory::Schema,
        res,
    )
}

async fn download_sha256(client: &HttpClient, url: &str) -> Result<String> {
    let resp = client.get_builder(url).send().await?;
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

async fn poll_tes_complete(
    cfg: &TestConfig,
    client: &HttpClient,
    task_id: &str,
) -> Result<serde_json::Value> {
    let status_url = format!(
        "{}/tasks/{}",
        cfg.services.tes_url.trim_end_matches('/'),
        task_id
    );
    let start = std::time::Instant::now();
    let timeout = Duration::from_secs(300);
    loop {
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
            if state != "COMPLETE" {
                anyhow::bail!("Expected TES task to COMPLETE, got {state}");
            }
            return Ok(v);
        }
        if start.elapsed() > timeout {
            anyhow::bail!("Timed out waiting for TES task {}", task_id);
        }
        sleep(Duration::from_secs(2)).await;
    }
}

async fn level2_task_lifecycle_allowlist(
    cfg: &TestConfig,
    client: &HttpClient,
    al: &AllowlistExec,
) -> TestCaseResult {
    let res = async {
        let url = format!("{}/tasks", cfg.services.tes_url.trim_end_matches('/'));
        let body = al.tes_create_body();
        let (status, text) = client.post_raw(&url, &body).await?;
        if !status.is_success() {
            anyhow::bail!("TES allowlist task submission failed: HTTP {status}: {text}");
        }
        let v: serde_json::Value = serde_json::from_str(&text)?;
        let task_id = v
            .get("id")
            .and_then(|x| x.as_str())
            .ok_or_else(|| anyhow::anyhow!("TES createTask response missing id: {}", v))?
            .to_owned();

        info!(%task_id, "Submitted TES allowlist task");
        let task_json = poll_tes_complete(cfg, client, &task_id).await?;

        let outs = tes_output_urls(&task_json);
        if outs.is_empty() {
            anyhow::bail!("TES COMPLETE but outputs[] empty: {}", task_json);
        }
        let mut verified = false;
        for out in &outs {
            if let Some(rest) = out.strip_prefix("drs://") {
                let object_id = rest.trim_start_matches('/');
                let drs_url = format!(
                    "{}/objects/{}",
                    cfg.services.drs_url.trim_end_matches('/'),
                    object_id
                );
                let drs_obj = client.get_json(&drs_url).await?;
                if drs_obj.get("id").and_then(|x| x.as_str()).is_some() {
                    verified = true;
                    break;
                }
            } else {
                // HTTP(S) download URL
                match download_sha256(client, out).await {
                    Ok(_) => {
                        verified = true;
                        break;
                    }
                    Err(e) => info!(url = %out, error = %e, "TES output URL not downloadable"),
                }
            }
        }
        if !verified {
            anyhow::bail!(
                "TES COMPLETE but no resolvable DRS/HTTP output among {:?}",
                outs
            );
        }
        Ok::<(), anyhow::Error>(())
    }
    .await;

    TestCaseResult::from_outcome(
        "TES allowlist task lifecycle (COMPLETE + DRS/HTTP output)",
        ComplianceLevel::Level2,
        TestCategory::Lifecycle,
        res,
    )
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
        let resp = client.post_builder(&url).json(&body).send().await?;
        if !resp.status().is_success() {
            anyhow::bail!("TES task submission failed: {}", resp.status());
        }
        let v: serde_json::Value = resp.json().await?;
        let task_id = v
            .get("id")
            .and_then(|x| x.as_str())
            .ok_or_else(|| anyhow::anyhow!("TES createTask response missing id: {}", v))?;

        info!(%task_id, "Submitted TES task for lifecycle + checksum test");

        let task_json = poll_tes_complete(cfg, client, task_id).await?;

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
        TestCategory::Lifecycle,
        res,
    )
}
