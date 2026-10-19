// SPDX-License-Identifier: Apache-2.0
use anyhow::Result;
use common::config::TestConfig;
use common::ga4gh_schemas;
use common::http::HttpClient;
use common::report::{ComplianceLevel, ServiceKind, ServiceReport, TestCaseResult, TestCategory};
use common::workflow::{
    fetch_wes_run_output, poll_wes_run_until_terminal, submit_wes_run, submit_wes_run_status,
    WesRunRequest,
};
use std::time::Duration;

use crate::allowlist::AllowlistExec;
use crate::{level0_http, Features, Mode};

pub async fn run_wes_checks(
    _mode: Mode,
    features: &Features,
    cfg: &TestConfig,
    client: &HttpClient,
) -> Result<ServiceReport> {
    let mut tests = Vec::new();

    tests.push(level0_service_info_reachable(cfg, client).await);
    tests.push(level1_service_info_schema(cfg, client).await);

    if features.execution_allowlist {
        let al = match AllowlistExec::from_env() {
            Ok(a) => a,
            Err(e) => {
                let msg = e.to_string();
                tests.push(TestCaseResult::fail(
                    "WES allowlist env (fail closed)",
                    ComplianceLevel::Level2,
                    TestCategory::Lifecycle,
                    msg.clone(),
                ));
                tests.push(TestCaseResult::fail(
                    "WES lifecycle success allowlist",
                    ComplianceLevel::Level2,
                    TestCategory::Lifecycle,
                    msg.clone(),
                ));
                tests.push(TestCaseResult::fail(
                    "WES non-allowlisted submit denied (403)",
                    ComplianceLevel::Level2,
                    TestCategory::Lifecycle,
                    msg,
                ));
                tests.push(level2_scatter_gather(features, cfg, client).await);
                return Ok(ServiceReport {
                    service: ServiceKind::Wes,
                    tests,
                });
            }
        };
        tests.push(level2_lifecycle_success_allowlist(cfg, client, &al).await);
        tests.push(level2_non_allowlisted_denied(cfg, client).await);
        tests.push(level2_scatter_gather(features, cfg, client).await);
    } else {
        tests.push(level2_lifecycle_success(cfg, client).await);
        tests.push(level2_failure_state(cfg, client).await);
        tests.push(level2_missing_inputs_error_state(cfg, client).await);
        tests.push(level2_incompatible_type_error_state(cfg, client).await);
        tests.push(level3_invalid_workflow(cfg, client).await);
        tests.push(level2_scatter_gather(features, cfg, client).await);
    }

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
    level0_http(
        "WES service-info reachable",
        client.inner().get(&url).send().await,
    )
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
            let ok_version = v
                .get("supported_wes_versions")
                .and_then(|x| x.as_array())
                .and_then(|a| {
                    a.iter()
                        .filter_map(|v| v.as_str())
                        .find(|s| *s == "1.0" || *s == "1.1")
                });
            if ok_version.is_none() {
                errors.push("supported_wes_versions must contain at least 1.0 or 1.1".to_string());
            }
            if errors.is_empty() {
                TestCaseResult::pass(
                    "WES service-info schema (GA4GH official)",
                    ComplianceLevel::Level1,
                    TestCategory::Schema,
                )
            } else {
                TestCaseResult::fail(
                    "WES service-info schema (GA4GH official)",
                    ComplianceLevel::Level1,
                    TestCategory::Schema,
                    errors.join("; "),
                )
            }
        }
        Err(e) => TestCaseResult::fail(
            "WES service-info schema (GA4GH official)",
            ComplianceLevel::Level1,
            TestCategory::Schema,
            e,
        ),
    }
}

async fn poll_echo(
    cfg: &TestConfig,
    client: &HttpClient,
    req: WesRunRequest,
    timeout: Duration,
) -> anyhow::Result<common::workflow::WesRunStatus> {
    let run_id = submit_wes_run(client, &cfg.services.wes_url, &req).await?;
    poll_wes_run_until_terminal(
        client,
        &cfg.services.wes_url,
        &run_id,
        timeout,
        Duration::from_secs(2),
    )
    .await
}

async fn level2_lifecycle_success_allowlist(
    cfg: &TestConfig,
    client: &HttpClient,
    al: &AllowlistExec,
) -> TestCaseResult {
    let req = WesRunRequest {
        workflow_url: al.wes_workflow_id.clone(),
        workflow_type: "s4-allowlist".to_owned(),
        workflow_type_version: "1".to_owned(),
        tags: None,
        workflow_params: serde_json::json!({}),
    };
    let result = async {
        let status = poll_echo(cfg, client, req, Duration::from_secs(300)).await?;
        if status.state != "COMPLETE" {
            anyhow::bail!(
                "Expected COMPLETE, got {} (states: {:?})",
                status.state,
                status.states_history
            );
        }
        let saw_pre_terminal = status.states_history.iter().any(|s| {
            matches!(s.as_str(), "QUEUED" | "INITIALIZING" | "RUNNING")
        });
        if !saw_pre_terminal {
            anyhow::bail!(
                "WES lifecycle for success run must include at least one of QUEUED, INITIALIZING, RUNNING before COMPLETE; got {:?}",
                status.states_history
            );
        }
        let outputs = fetch_wes_run_output(client, &cfg.services.wes_url, &status.run_id).await?;
        let result_uri = outputs
            .get("result")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing outputs.result (drs://…) in: {}", outputs))?;
        if !result_uri.starts_with("drs://") {
            anyhow::bail!("outputs.result must be a drs:// URI, got {result_uri}");
        }
        let object_id = result_uri
            .strip_prefix("drs://")
            .unwrap_or(result_uri)
            .trim_start_matches('/');
        let drs_url = format!(
            "{}/objects/{}",
            cfg.services.drs_url.trim_end_matches('/'),
            object_id
        );
        let drs_obj = client.get_json(&drs_url).await?;
        if drs_obj.get("id").and_then(|v| v.as_str()).is_none() {
            anyhow::bail!("DRS output missing id: {}", drs_obj);
        }
        Ok::<(), anyhow::Error>(())
    }
    .await;
    TestCaseResult::from_outcome(
        "WES lifecycle success allowlist (API may show QUEUED/INITIALIZING/RUNNING before COMPLETE)",
        ComplianceLevel::Level2,
        TestCategory::Lifecycle,
        result,
    )
}

async fn level2_non_allowlisted_denied(cfg: &TestConfig, client: &HttpClient) -> TestCaseResult {
    // One representative non-allowlisted URL covers fail / invalid / stub fixtures.
    let cases = [
        ("trs://test-tool/echo/1.0", "CWL", "v1.2"),
        ("trs://test-tool/fail/1.0", "CWL", "v1.2"),
        ("trs://nonexistent/invalid/0.0", "CWL", "v1.2"),
    ];
    let result = async {
        for (url, wtype, wver) in cases {
            let req = WesRunRequest {
                workflow_url: url.to_owned(),
                workflow_type: wtype.to_owned(),
                workflow_type_version: wver.to_owned(),
                tags: None,
                workflow_params: serde_json::json!({}),
            };
            let (status, body) = submit_wes_run_status(client, &cfg.services.wes_url, &req).await?;
            if status != 403 {
                anyhow::bail!(
                    "non-allowlisted workflow_url {url} expected HTTP 403, got {status}: {body}"
                );
            }
        }
        Ok::<(), anyhow::Error>(())
    }
    .await;
    TestCaseResult::from_outcome(
        "WES non-allowlisted submit denied (403)",
        ComplianceLevel::Level2,
        TestCategory::Lifecycle,
        result,
    )
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
        let status = poll_echo(cfg, client, req, Duration::from_secs(300)).await?;
        if status.state != "COMPLETE" {
            anyhow::bail!(
                "Expected COMPLETE, got {} (states: {:?})",
                status.state,
                status.states_history
            );
        }
        let saw_pre_terminal = status.states_history.iter().any(|s| {
            matches!(s.as_str(), "QUEUED" | "INITIALIZING" | "RUNNING")
        });
        if !saw_pre_terminal {
            anyhow::bail!(
                "WES lifecycle for success run must include at least one of QUEUED, INITIALIZING, RUNNING before COMPLETE; got {:?}",
                status.states_history
            );
        }
        let outputs = fetch_wes_run_output(client, &cfg.services.wes_url, &status.run_id).await?;
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
    TestCaseResult::from_outcome(
        "WES lifecycle success echo (API may show QUEUED/INITIALIZING/RUNNING before COMPLETE)",
        ComplianceLevel::Level2,
        TestCategory::Lifecycle,
        result,
    )
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
        let status = poll_echo(cfg, client, req, Duration::from_secs(300)).await?;
        if status.state != "EXECUTOR_ERROR" && status.state != "SYSTEM_ERROR" {
            anyhow::bail!("Expected error state, got {}", status.state);
        }
        Ok::<(), anyhow::Error>(())
    }
    .await;
    TestCaseResult::from_outcome(
        "WES failure state for bad workflow",
        ComplianceLevel::Level2,
        TestCategory::Lifecycle,
        result,
    )
}

async fn level2_missing_inputs_error_state(
    cfg: &TestConfig,
    client: &HttpClient,
) -> TestCaseResult {
    let req = WesRunRequest {
        workflow_url: "trs://test-tool/cwl-echo/1.0".to_owned(),
        workflow_type: "CWL".to_owned(),
        workflow_type_version: "v1.2".to_owned(),
        tags: None,
        workflow_params: serde_json::json!({}),
    };
    let result = async {
        let status = poll_echo(cfg, client, req, Duration::from_secs(300)).await?;
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
    TestCaseResult::from_outcome(
        "WES missing inputs leads to error state",
        ComplianceLevel::Level2,
        TestCategory::Lifecycle,
        result,
    )
}

async fn level2_incompatible_type_error_state(
    cfg: &TestConfig,
    client: &HttpClient,
) -> TestCaseResult {
    let req = WesRunRequest {
        workflow_url: "trs://test-tool/cwl-echo/1.0".to_owned(),
        workflow_type: "WDL".to_owned(),
        workflow_type_version: "1.0".to_owned(),
        tags: None,
        workflow_params: serde_json::json!({ "message": "hello-type-mismatch" }),
    };
    let result = async {
        let status = poll_echo(cfg, client, req, Duration::from_secs(300)).await?;
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
    TestCaseResult::from_outcome(
        "WES incompatible workflow_type leads to error state",
        ComplianceLevel::Level2,
        TestCategory::Lifecycle,
        result,
    )
}

async fn level3_invalid_workflow(cfg: &TestConfig, client: &HttpClient) -> TestCaseResult {
    let req = WesRunRequest {
        workflow_url: "trs://nonexistent/invalid/0.0".to_owned(),
        workflow_type: "CWL".to_owned(),
        workflow_type_version: "v1.2".to_owned(),
        tags: None,
        workflow_params: serde_json::json!({}),
    };
    let result = async {
        let status = poll_echo(cfg, client, req, Duration::from_secs(300)).await?;
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
    TestCaseResult::from_outcome(
        "WES invalid workflow leads to error state",
        ComplianceLevel::Level3,
        TestCategory::Other,
        result,
    )
}

async fn level2_scatter_gather(
    features: &Features,
    cfg: &TestConfig,
    client: &HttpClient,
) -> TestCaseResult {
    const NAME: &str = "WES scatter/gather workflow";
    if !features.supports_scatter_gather {
        return TestCaseResult::skip(
            NAME,
            ComplianceLevel::Level2,
            TestCategory::WorkflowCorrectness,
            "supports_scatter_gather=false in features",
        );
    }
    let req = WesRunRequest {
        workflow_url: "trs://test-tool/scatter-gather/1.0".to_owned(),
        workflow_type: "CWL".to_owned(),
        workflow_type_version: "v1.2".to_owned(),
        tags: None,
        workflow_params: serde_json::json!({ "items": [1, 2, 3, 4] }),
    };
    let result = async {
        let run_id = submit_wes_run(client, &cfg.services.wes_url, &req).await?;
        let status = poll_wes_run_until_terminal(
            client,
            &cfg.services.wes_url,
            &run_id,
            Duration::from_secs(600),
            Duration::from_secs(5),
        )
        .await?;
        if status.state != "COMPLETE" {
            anyhow::bail!("Scatter/gather expected COMPLETE, got {}", status.state);
        }
        let outputs = fetch_wes_run_output(client, &cfg.services.wes_url, &run_id).await?;
        if outputs.get("scatter_result").is_none() {
            anyhow::bail!("Missing scatter_result in outputs: {}", outputs);
        }
        Ok::<(), anyhow::Error>(())
    }
    .await;
    TestCaseResult::from_outcome(
        NAME,
        ComplianceLevel::Level2,
        TestCategory::WorkflowCorrectness,
        result,
    )
}
