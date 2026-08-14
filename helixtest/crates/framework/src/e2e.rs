//! Cross-service interoperability checks (Level 3): TRS → DRS → WES → DRS output → Beacon.
//!
//! **Scope:** This module drives WES to **terminal `COMPLETE`** via `common::workflow::poll_wes_run_until_terminal`.
//! It does **not** poll TES. Full TRS→…→TES coupling lives in the `e2e-tests` crate only when
//! the mock stack defines that contract.

use anyhow::{Context, Result};
use common::config::TestConfig;
use common::http::HttpClient;
use common::report::{ComplianceLevel, ServiceKind, ServiceReport, TestCaseResult, TestCategory};
use common::util::{sha256_bytes, test_data_dir};
use common::workflow::{
    fetch_wes_run_output, poll_wes_run_until_terminal, submit_wes_run, WesRunRequest,
};
use serde_json::Value;
use std::time::Duration;
use url::Url;

use crate::{level0_http, Features, Mode};

fn preferred_drs_input_uri(drs_obj: &Value, drs_id: &str) -> String {
    if let Some(self_uri) = drs_obj.get("self_uri").and_then(|v| v.as_str()) {
        if self_uri.starts_with("drs://") {
            return self_uri.to_string();
        }
    }
    format!("drs://{}", drs_id)
}

pub async fn run_e2e_checks(
    _mode: Mode,
    _features: &Features,
    cfg: &TestConfig,
    client: &HttpClient,
) -> Result<ServiceReport> {
    let mut tests = Vec::new();
    let tools_url = format!("{}/tools", cfg.services.trs_url.trim_end_matches('/'));
    tests.push(level0_http(
        "E2E TRS /tools reachable",
        client.inner().get(&tools_url).send().await,
    ));
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
    TestCaseResult::from_outcome(
        "E2E TRS→DRS→WES→DRS output→Beacon (WES polled to terminal; no TES poll in this module)",
        ComplianceLevel::Level3,
        TestCategory::Interoperability,
        result,
    )
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
    let req = WesRunRequest {
        workflow_url: trs_workflow_url,
        workflow_type: "CWL".to_owned(),
        workflow_type_version: "v1.2".to_owned(),
        tags: None,
        workflow_params: serde_json::json!({
            "input_drs_uri": preferred_drs_input_uri(&drs_obj, drs_id)
        }),
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
    let actual_checksum = sha256_bytes(&bytes);

    let expected_path = test_data_dir()?
        .join("expected")
        .join("e2e")
        .join("result.txt.sha256");
    let expected_checksum = std::fs::read_to_string(&expected_path)
        .with_context(|| {
            format!(
                "E2E golden checksum missing at {} — add the expected SHA-256; refusing to treat actual as expected",
                expected_path.display()
            )
        })?
        .trim()
        .to_owned();
    if !actual_checksum.eq_ignore_ascii_case(&expected_checksum) {
        anyhow::bail!(
            "E2E result checksum mismatch: expected {}, got {}",
            expected_checksum,
            actual_checksum
        );
    }

    let beacon_url = format!("{}/query", cfg.services.beacon_url.trim_end_matches('/'));
    let beacon_resp = client
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
    let exists = beacon_resp
        .pointer("/response/exists")
        .and_then(|v| v.as_bool());
    if exists == Some(false) {
        anyhow::bail!("Beacon reported exists=false after E2E pipeline");
    }

    Ok(())
}
