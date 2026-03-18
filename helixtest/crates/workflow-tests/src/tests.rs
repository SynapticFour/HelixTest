use anyhow::Result;
use common::config::TestConfig;
use common::http::HttpClient;
use common::util::sha256_file;
use common::workflow::{
    fetch_wes_run_output, poll_wes_run_until_terminal, submit_wes_run, WesRunRequest,
};
use std::path::PathBuf;
use std::time::Duration;

fn test_data_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("test-data")
}

struct WorkflowCase<'a> {
    name: &'a str,
    workflow_type: &'a str,
    workflow_type_version: &'a str,
    workflow_url: &'a str,
    output_field: &'a str,
    expected_output_file: &'a str,
    expected_checksum_file: &'a str,
}

fn workflow_cases<'a>() -> Vec<WorkflowCase<'a>> {
    vec![
        WorkflowCase {
            name: "cwl_echo",
            workflow_type: "CWL",
            workflow_type_version: "v1.2",
            workflow_url: "trs://test-tool/cwl-echo/1.0",
            output_field: "echo_out",
            expected_output_file: "workflows/outputs/cwl_echo_out.txt",
            expected_checksum_file: "expected/workflows/cwl_echo_out.txt.sha256",
        },
        WorkflowCase {
            name: "wdl_echo",
            workflow_type: "WDL",
            workflow_type_version: "1.0",
            workflow_url: "trs://test-tool/wdl-echo/1.0",
            output_field: "echo_out",
            expected_output_file: "workflows/outputs/wdl_echo_out.txt",
            expected_checksum_file: "expected/workflows/wdl_echo_out.txt.sha256",
        },
        WorkflowCase {
            name: "nextflow_echo",
            workflow_type: "NFL",
            workflow_type_version: "20.10.0",
            workflow_url: "trs://test-tool/nextflow-echo/1.0",
            output_field: "echo_out",
            expected_output_file: "workflows/outputs/nextflow_echo_out.txt",
            expected_checksum_file: "expected/workflows/nextflow_echo_out.txt.sha256",
        },
    ]
}

async fn run_workflow_case(case: &WorkflowCase<'_>) -> Result<()> {
    let cfg = TestConfig::from_env_or_file()?;
    let client = HttpClient::new();

    let req = WesRunRequest {
        workflow_url: case.workflow_url.to_owned(),
        workflow_type: case.workflow_type.to_owned(),
        workflow_type_version: case.workflow_type_version.to_owned(),
        tags: None,
        workflow_params: serde_json::json!({
            "message": format!("hello-{}", case.name)
        }),
    };

    let run_id = submit_wes_run(&client, &cfg.services.wes_url, &req).await?;
    let status = poll_wes_run_until_terminal(
        &client,
        &cfg.services.wes_url,
        &run_id,
        Duration::from_secs(600),
        Duration::from_secs(5),
    )
    .await?;

    if status.state != "COMPLETE" {
        anyhow::bail!(
            "Workflow {} expected COMPLETE, got {}",
            case.name,
            status.state
        );
    }

    let outputs = fetch_wes_run_output(&client, &cfg.services.wes_url, &run_id).await?;
    let echoed = outputs
        .get(case.output_field)
        .and_then(|v| v.as_str())
        .ok_or_else(|| {
            anyhow::anyhow!(
                "Missing {} in outputs for {}: {}",
                case.output_field,
                case.name,
                outputs
            )
        })?;
    if echoed != format!("hello-{}", case.name) {
        anyhow::bail!(
            "Workflow {} echo_out mismatch: expected {}, got {}",
            case.name,
            format!("hello-{}", case.name),
            echoed
        );
    }

    let root = test_data_dir();
    let expected_checksum_path = root.join(case.expected_checksum_file);
    let expected_checksum = std::fs::read_to_string(&expected_checksum_path)?
        .trim()
        .to_owned();

    let produced_file = root.join(case.expected_output_file);
    let actual_checksum = sha256_file(&produced_file)?;
    if !actual_checksum.eq_ignore_ascii_case(&expected_checksum) {
        anyhow::bail!(
            "Workflow {} output checksum mismatch: expected {}, got {}",
            case.name,
            expected_checksum,
            actual_checksum
        );
    }

    Ok(())
}

#[tokio::test]
async fn cwl_wdl_nextflow_echo_workflows_are_deterministic() -> Result<()> {
    for case in workflow_cases() {
        run_workflow_case(&case).await?;
    }
    Ok(())
}

#[tokio::test]
async fn failing_workflow_ends_in_error_state() -> Result<()> {
    let cfg = TestConfig::from_env_or_file()?;
    let client = HttpClient::new();

    let req = WesRunRequest {
        workflow_url: "trs://test-tool/fail/1.0".to_owned(),
        workflow_type: "CWL".to_owned(),
        workflow_type_version: "v1.2".to_owned(),
        tags: None,
        workflow_params: serde_json::json!({}),
    };

    let run_id = submit_wes_run(&client, &cfg.services.wes_url, &req).await?;
    let status = poll_wes_run_until_terminal(
        &client,
        &cfg.services.wes_url,
        &run_id,
        Duration::from_secs(600),
        Duration::from_secs(5),
    )
    .await?;

    assert!(
        matches!(status.state.as_str(), "EXECUTOR_ERROR" | "SYSTEM_ERROR"),
        "Failing workflow must end in error state, got {}",
        status.state
    );

    Ok(())
}

#[tokio::test]
async fn scatter_gather_workflow_produces_expected_checksum() -> Result<()> {
    let cfg = TestConfig::from_env_or_file()?;
    let client = HttpClient::new();

    let case = WorkflowCase {
        name: "scatter_gather",
        workflow_type: "CWL",
        workflow_type_version: "v1.2",
        workflow_url: "trs://test-tool/scatter-gather/1.0",
        output_field: "scatter_result",
        expected_output_file: "workflows/outputs/scatter_gather_out.txt",
        expected_checksum_file: "expected/workflows/scatter_gather_out.txt.sha256",
    };

    let req = WesRunRequest {
        workflow_url: case.workflow_url.to_owned(),
        workflow_type: case.workflow_type.to_owned(),
        workflow_type_version: case.workflow_type_version.to_owned(),
        tags: None,
        workflow_params: serde_json::json!({
            "items": [1, 2, 3, 4]
        }),
    };

    let run_id = submit_wes_run(&client, &cfg.services.wes_url, &req).await?;
    let status = poll_wes_run_until_terminal(
        &client,
        &cfg.services.wes_url,
        &run_id,
        Duration::from_secs(600),
        Duration::from_secs(5),
    )
    .await?;

    assert_eq!(
        status.state, "COMPLETE",
        "Scatter/gather workflow must complete successfully"
    );

    let outputs = fetch_wes_run_output(&client, &cfg.services.wes_url, &run_id).await?;
    let _ = outputs.get(case.output_field).ok_or_else(|| {
        anyhow::anyhow!(
            "Missing {} in outputs for scatter/gather: {}",
            case.output_field,
            outputs
        )
    })?;

    let root = test_data_dir();
    let expected_checksum_path = root.join(case.expected_checksum_file);
    let expected_checksum = std::fs::read_to_string(&expected_checksum_path)?
        .trim()
        .to_owned();

    let produced_file = root.join(case.expected_output_file);
    let actual_checksum = sha256_file(&produced_file)?;
    assert_eq!(
        actual_checksum.to_lowercase(),
        expected_checksum.to_lowercase(),
        "Scatter/gather workflow output checksum mismatch"
    );

    Ok(())
}


