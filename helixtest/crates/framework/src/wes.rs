use anyhow::Result;
use common::config::TestConfig;
use common::ga4gh_schemas;
use common::http::HttpClient;
use common::report::{ComplianceLevel, ServiceKind, ServiceReport, TestCategory, TestCaseResult};
use common::workflow::{
    fetch_wes_run_output, poll_wes_run_until_terminal, submit_wes_run, WesRunRequest,
};
use std::time::Duration;

use crate::{Features, Mode};

pub async fn run_wes_checks(
    _mode: Mode,
    _features: &Features,
    cfg: &TestConfig,
    client: &HttpClient,
) -> Result<ServiceReport> {
    let mut tests = Vec::new();

    // Level 0: API reachable
    tests.push(level0_service_info_reachable(cfg, client).await);
    // Level 1: schema compliance
    tests.push(level1_service_info_schema(cfg, client).await);
    // Level 2: functional correctness (lifecycle + success output)
    tests.push(level2_lifecycle_success(cfg, client).await);
    tests.push(level2_failure_state(cfg, client).await);
    tests.push(level2_missing_inputs_error_state(cfg, client).await);
    tests.push(level2_incompatible_type_error_state(cfg, client).await);
    // Level 3: invalid workflow handling
    tests.push(level3_invalid_workflow(cfg, client).await);
    // Robustness: timeout and retry behavior
    tests.push(robustness_polling_timeout_yields_clear_error(cfg, client).await);

    Ok(ServiceReport {
        service: ServiceKind::Wes,
        tests,
    })
}

async fn level0_service_info_reachable(cfg: &TestConfig, client: &HttpClient) -> TestCaseResult {
    let url = format!(
        "{}/service-info",
        cfg.services.wes_url.trim_end_matches('/')
    );
    let res = client.inner().get(&url).send().await;
    match res {
        Ok(resp) => TestCaseResult {
            name: "WES service-info reachable".into(),
            level: ComplianceLevel::Level0,
            passed: resp.status().is_success(),
            error: (!resp.status().is_success())
                .then(|| format!("HTTP {}", resp.status())),
            category: TestCategory::Other,
            weight: 1.0,
        },
        Err(e) => TestCaseResult {
            name: "WES service-info reachable".into(),
            level: ComplianceLevel::Level0,
            passed: false,
            error: Some(e.to_string()),
            category: TestCategory::Other,
            weight: 1.0,
        },
    }
}

async fn level1_service_info_schema(cfg: &TestConfig, client: &HttpClient) -> TestCaseResult {
    let url = format!(
        "{}/service-info",
        cfg.services.wes_url.trim_end_matches('/')
    );
    let res = client.get_json(&url).await;
    match res {
        Ok(v) => {
            let mut errors = Vec::new();
            if let Err(e) = ga4gh_schemas::validate_wes_service_info(&v) {
                errors.push(e.to_string());
            }
            // GA4GH WES conformance: at least one supported version must be 1.0 or 1.1
            let ok_version = v
                .get("supported_wes_versions")
                .and_then(|x| x.as_array())
                .and_then(|a| {
                    a.iter()
                        .filter_map(|v| v.as_str())
                        .find(|s| *s == "1.0" || *s == "1.1")
                });
            if ok_version.is_none() {
                errors.push(
                    "supported_wes_versions must contain at least 1.0 or 1.1".to_string(),
                );
            }
            TestCaseResult {
                name: "WES service-info schema (GA4GH official)".into(),
                level: ComplianceLevel::Level1,
                passed: errors.is_empty(),
                error: (!errors.is_empty()).then(|| errors.join("; ")),
                category: TestCategory::Schema,
                weight: 1.0,
            }
        }
        Err(e) => TestCaseResult {
            name: "WES service-info schema (GA4GH official)".into(),
            level: ComplianceLevel::Level1,
            passed: false,
            error: Some(e.to_string()),
            category: TestCategory::Schema,
            weight: 1.0,
        },
    }
}

async fn level2_lifecycle_success(cfg: &TestConfig, client: &HttpClient) -> TestCaseResult {
    let req = WesRunRequest {
        workflow_url: "trs://test-tool/echo/1.0".to_owned(),
        workflow_type: "CWL".to_owned(),
        workflow_type_version: "v1.2".to_owned(),
        tags: None,
        workflow_params: serde_json::json!({ "message": "hello-ga4gh" }),
    };
    let result = async {
        let run_id = submit_wes_run(client, &cfg.services.wes_url, &req).await?;
        let status = poll_wes_run_until_terminal(
            client,
            &cfg.services.wes_url,
            &run_id,
            Duration::from_secs(300),
            Duration::from_secs(2),
        )
        .await?;
        if status.state != "COMPLETE" {
            anyhow::bail!(
                "Expected COMPLETE, got {} (states: {:?})",
                status.state,
                status.states_history
            );
        }
        // Ensure we saw at least QUEUED and RUNNING along the way
        let saw_queued = status.states_history.iter().any(|s| s == "QUEUED");
        let saw_running = status.states_history.iter().any(|s| s == "RUNNING");
        if !saw_queued || !saw_running {
            anyhow::bail!(
                "WES lifecycle for success run must include QUEUED and RUNNING; got {:?}",
                status.states_history
            );
        }
        let outputs = fetch_wes_run_output(client, &cfg.services.wes_url, &run_id).await?;
        let echoed = outputs
            .get("echo_out")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing echo_out in outputs: {}", outputs))?;
        if echoed != "hello-ga4gh" {
            anyhow::bail!("echo_out mismatch: got {}", echoed);
        }
        Ok::<(), anyhow::Error>(())
    }
    .await;
    TestCaseResult {
        name: "WES lifecycle success echo".into(),
        level: ComplianceLevel::Level2,
        passed: result.is_ok(),
        error: result.err().map(|e| e.to_string()),
        category: TestCategory::Lifecycle,
        weight: 1.0,
    }
}

async fn level2_failure_state(cfg: &TestConfig, client: &HttpClient) -> TestCaseResult {
    let req = WesRunRequest {
        workflow_url: "trs://test-tool/fail/1.0".to_owned(),
        workflow_type: "CWL".to_owned(),
        workflow_type_version: "v1.2".to_owned(),
        tags: None,
        workflow_params: serde_json::json!({}),
    };
    let result = async {
        let run_id = submit_wes_run(client, &cfg.services.wes_url, &req).await?;
        let status = poll_wes_run_until_terminal(
            client,
            &cfg.services.wes_url,
            &run_id,
            Duration::from_secs(300),
            Duration::from_secs(2),
        )
        .await?;
        if status.state != "EXECUTOR_ERROR" && status.state != "SYSTEM_ERROR" {
            anyhow::bail!("Expected error state, got {}", status.state);
        }
        Ok::<(), anyhow::Error>(())
    }
    .await;
    TestCaseResult {
        name: "WES failure state for bad workflow".into(),
        level: ComplianceLevel::Level2,
        passed: result.is_ok(),
        error: result.err().map(|e| e.to_string()),
        category: TestCategory::Lifecycle,
        weight: 1.0,
    }
}

async fn level2_missing_inputs_error_state(cfg: &TestConfig, client: &HttpClient) -> TestCaseResult {
    // Use a valid workflow but omit required input parameters
    let req = WesRunRequest {
        workflow_url: "trs://test-tool/cwl-echo/1.0".to_owned(),
        workflow_type: "CWL".to_owned(),
        workflow_type_version: "v1.2".to_owned(),
        tags: None,
        workflow_params: serde_json::json!({}), // missing "message"
    };
    let result = async {
        let run_id = submit_wes_run(client, &cfg.services.wes_url, &req).await?;
        let status = poll_wes_run_until_terminal(
            client,
            &cfg.services.wes_url,
            &run_id,
            Duration::from_secs(300),
            Duration::from_secs(2),
        )
        .await?;
        if status.state != "EXECUTOR_ERROR" && status.state != "SYSTEM_ERROR" {
            anyhow::bail!(
                "Missing-input workflow expected EXECUTOR_ERROR or SYSTEM_ERROR, got {} (states: {:?})",
                status.state,
                status.states_history
            );
        }
        Ok::<(), anyhow::Error>(())
    }
    .await;
    TestCaseResult {
        name: "WES missing inputs leads to error state".into(),
        level: ComplianceLevel::Level2,
        passed: result.is_ok(),
        error: result.err().map(|e| e.to_string()),
        category: TestCategory::Lifecycle,
        weight: 1.0,
    }
}

async fn level2_incompatible_type_error_state(cfg: &TestConfig, client: &HttpClient) -> TestCaseResult {
    // Use a CWL workflow but declare an incompatible workflow_type
    let req = WesRunRequest {
        workflow_url: "trs://test-tool/cwl-echo/1.0".to_owned(),
        workflow_type: "WDL".to_owned(), // intentionally wrong
        workflow_type_version: "1.0".to_owned(),
        tags: None,
        workflow_params: serde_json::json!({ "message": "hello-type-mismatch" }),
    };
    let result = async {
        let run_id = submit_wes_run(client, &cfg.services.wes_url, &req).await?;
        let status = poll_wes_run_until_terminal(
            client,
            &cfg.services.wes_url,
            &run_id,
            Duration::from_secs(300),
            Duration::from_secs(2),
        )
        .await?;
        if status.state != "EXECUTOR_ERROR" && status.state != "SYSTEM_ERROR" {
            anyhow::bail!(
                "Incompatible-type workflow expected EXECUTOR_ERROR or SYSTEM_ERROR, got {} (states: {:?})",
                status.state,
                status.states_history
            );
        }
        Ok::<(), anyhow::Error>(())
    }
    .await;
    TestCaseResult {
        name: "WES incompatible workflow_type leads to error state".into(),
        level: ComplianceLevel::Level2,
        passed: result.is_ok(),
        error: result.err().map(|e| e.to_string()),
        category: TestCategory::Lifecycle,
        weight: 1.0,
    }
}

async fn level3_invalid_workflow(cfg: &TestConfig, client: &HttpClient) -> TestCaseResult {
    // This test assumes the implementation accepts the run but fails it during execution
    // due to an invalid or non-existent workflow descriptor.
    let req = WesRunRequest {
        workflow_url: "trs://nonexistent/invalid/0.0".to_owned(),
        workflow_type: "CWL".to_owned(),
        workflow_type_version: "v1.2".to_owned(),
        tags: None,
        workflow_params: serde_json::json!({}),
    };
    let result = async {
        let run_id = submit_wes_run(client, &cfg.services.wes_url, &req).await?;
        let status = poll_wes_run_until_terminal(
            client,
            &cfg.services.wes_url,
            &run_id,
            Duration::from_secs(300),
            Duration::from_secs(2),
        )
        .await?;
        if status.state != "EXECUTOR_ERROR" && status.state != "SYSTEM_ERROR" {
            anyhow::bail!(
                "Invalid workflow run expected EXECUTOR_ERROR or SYSTEM_ERROR, got {} (states: {:?})",
                status.state,
                status.states_history
            );
        }
        Ok::<(), anyhow::Error>(())
    }
    .await;
    TestCaseResult {
        name: "WES invalid workflow leads to error state".into(),
        level: ComplianceLevel::Level3,
        passed: result.is_ok(),
        error: result.err().map(|e| e.to_string()),
        category: TestCategory::Other,
        weight: 1.0,
    }
}

/// Robustness: assert that when polling exceeds the timeout, the system returns a clear
/// error (no hang). Submits a run then polls with a 1s timeout; we expect a timeout error.
async fn robustness_polling_timeout_yields_clear_error(
    cfg: &TestConfig,
    client: &HttpClient,
) -> TestCaseResult {
    let req = WesRunRequest {
        workflow_url: "trs://test-tool/echo/1.0".to_owned(),
        workflow_type: "CWL".to_owned(),
        workflow_type_version: "v1.2".to_owned(),
        tags: None,
        workflow_params: serde_json::json!({ "message": "hello" }),
    };
    let result = async {
        let run_id = submit_wes_run(client, &cfg.services.wes_url, &req)
            .await
            .map_err(|e| e.to_string())?;
        poll_wes_run_until_terminal(
            client,
            &cfg.services.wes_url,
            &run_id,
            Duration::from_secs(1),
            Duration::from_millis(200),
        )
        .await
        .map_err(|e| e.to_string())
    }
    .await;
    let err_msg = result.err().unwrap_or_default();
    let passed = err_msg.to_lowercase().contains("timed out") || err_msg.to_lowercase().contains("timeout");
    TestCaseResult {
        name: "Robustness: polling timeout yields clear error".into(),
        level: ComplianceLevel::Level2,
        passed,
        error: if passed { None } else { Some(err_msg) },
        category: TestCategory::Robustness,
        weight: 1.0,
    }
}

