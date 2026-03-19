//! E2E interoperability: TRS → DRS → WES → TES → DRS → Beacon pipeline.

use anyhow::Result;
use common::config::TestConfig;
use common::http::HttpClient;
use common::report::{ComplianceLevel, ServiceKind, ServiceReport, TestCaseResult, TestCategory};
use common::workflow::{
    fetch_wes_run_output, poll_wes_run_until_terminal, submit_wes_run, WesRunRequest,
};
use serde_json::Value;
use sha2::Digest;
use std::path::PathBuf;
use std::time::Duration;
use url::Url;

use crate::{Features, Mode};

fn test_data_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("test-data")
}

pub async fn run_e2e_checks(
    _mode: Mode,
    _features: &Features,
    cfg: &TestConfig,
    client: &HttpClient,
) -> Result<ServiceReport> {
    let mut tests = Vec::new();
    tests.push(e2e_trs_drs_wes_tes_drs_beacon_pipeline(cfg, client).await);
    Ok(ServiceReport {
        service: ServiceKind::E2e,
        tests,
    })
}

async fn e2e_trs_drs_wes_tes_drs_beacon_pipeline(
    cfg: &TestConfig,
    client: &HttpClient,
) -> TestCaseResult {
    let result = run_e2e_pipeline(cfg, client).await;
    TestCaseResult {
        name: "E2E TRS→DRS→WES→TES→DRS→Beacon pipeline".into(),
        level: ComplianceLevel::Level3,
        passed: result.is_ok(),
        error: result.err().map(|e| e.to_string()),
        category: TestCategory::Interoperability,
        weight: 1.0,
    }
}

async fn run_e2e_pipeline(cfg: &TestConfig, client: &HttpClient) -> Result<()> {
    let tools_url = format!("{}/tools", cfg.services.trs_url.trim_end_matches('/'));
    let tools_val = client.get_json(&tools_url).await?;
    let tools = tools_val
        .as_array()
        .ok_or_else(|| anyhow::anyhow!("TRS /tools must return array"))?;
    let tool = tools
        .first()
        .ok_or_else(|| anyhow::anyhow!("TRS must expose at least one tool"))?;
    let tool_id = tool
        .get("id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("Tool missing id: {}", tool))?;

    let versions_url = format!(
        "{}/tools/{}/versions",
        cfg.services.trs_url.trim_end_matches('/'),
        tool_id
    );
    let versions_val = client.get_json(&versions_url).await?;
    let versions = versions_val
        .as_array()
        .ok_or_else(|| anyhow::anyhow!("TRS versions must return array"))?;
    let version = versions
        .first()
        .ok_or_else(|| anyhow::anyhow!("Tool must have at least one version"))?;
    let version_id = version
        .get("id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("ToolVersion missing id: {}", version))?;

    let drs_object_id = format!("{}-{}-input", tool_id, version_id);
    let drs_url = format!(
        "{}/objects/{}",
        cfg.services.drs_url.trim_end_matches('/'),
        drs_object_id
    );
    let drs_obj = client.get_json(&drs_url).await?;
    let drs_id = drs_obj
        .get("id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("DRS object missing id: {}", drs_obj))?;
    if drs_id != drs_object_id {
        anyhow::bail!(
            "DRS id mismatch: expected {}, got {}",
            drs_object_id,
            drs_id
        );
    }

    let trs_base = Url::parse(&cfg.services.trs_url)?;
    let host = trs_base
        .host_str()
        .ok_or_else(|| anyhow::anyhow!("TRS_URL missing host: {}", cfg.services.trs_url))?;
    let registry = if let Some(port) = trs_base.port() {
        format!("{}:{}", host, port)
    } else {
        host.to_string()
    };
    let trs_workflow_url = format!("trs://{}/{}/{}", registry, tool_id, version_id);
    let self_uri = drs_obj
        .get("self_uri")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .unwrap_or_else(|| format!("drs://{}", drs_id));
    let req = WesRunRequest {
        workflow_url: trs_workflow_url,
        workflow_type: "CWL".to_owned(),
        workflow_type_version: "v1.2".to_owned(),
        tags: None,
        workflow_params: serde_json::json!({ "input_drs_uri": Value::String(self_uri) }),
    };
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
        anyhow::bail!("E2E pipeline expected COMPLETE, got {}", status.state);
    }

    let outputs = fetch_wes_run_output(client, &cfg.services.wes_url, &run_id).await?;
    let drs_output_id = outputs
        .get("result_drs_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("Missing result_drs_id in outputs: {}", outputs))?;
    let drs_out_url = format!(
        "{}/objects/{}",
        cfg.services.drs_url.trim_end_matches('/'),
        drs_output_id
    );
    let drs_out_obj = client.get_json(&drs_out_url).await?;
    let access_methods = drs_out_obj
        .get("access_methods")
        .and_then(|x| x.as_array())
        .ok_or_else(|| anyhow::anyhow!("DRS output missing access_methods: {}", drs_out_obj))?;
    let first = &access_methods[0];
    let access_url = first
        .get("access_url")
        .and_then(|a| a.get("url"))
        .and_then(|x| x.as_str())
        .ok_or_else(|| {
            anyhow::anyhow!("access_methods[0].access_url.url missing: {}", drs_out_obj)
        })?;

    let resp = client.inner().get(access_url).send().await?;
    if !resp.status().is_success() {
        anyhow::bail!("Failed to download DRS output: {}", resp.status());
    }
    let bytes = resp.bytes().await?;
    let mut hasher = sha2::Sha256::new();
    hasher.update(&bytes);
    let actual_checksum = format!("{:x}", hasher.finalize());

    let expected_path = test_data_dir()
        .join("expected")
        .join("e2e")
        .join("result.txt.sha256");
    let expected_checksum = if expected_path.exists() {
        std::fs::read_to_string(&expected_path)?.trim().to_owned()
    } else {
        actual_checksum.clone()
    };
    if !actual_checksum.eq_ignore_ascii_case(&expected_checksum) {
        anyhow::bail!(
            "E2E result checksum mismatch: expected {}, got {}",
            expected_checksum,
            actual_checksum
        );
    }

    let beacon_url = format!("{}/query", cfg.services.beacon_url.trim_end_matches('/'));
    let _beacon_resp = client
        .post_json(
            &beacon_url,
            &serde_json::json!({
                "meta": { "apiVersion": "v2.0.0" },
                "query": {
                    "requestParameters": {
                        "referenceName": "1",
                        "start": 1000,
                        "referenceBases": "A",
                        "alternateBases": "T"
                    }
                }
            }),
        )
        .await?;

    Ok(())
}
