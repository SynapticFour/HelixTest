use anyhow::Result;
use common::config::TestConfig;
use common::http::HttpClient;
use common::workflow::{fetch_wes_run_output, poll_wes_run_until_terminal, submit_wes_run, WesRunRequest};
use serde_json::Value;
use url::Url;
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

#[tokio::test]
async fn full_trs_drs_wes_tes_beacon_pipeline() -> Result<()> {
    let cfg = TestConfig::from_env_or_file()?;
    let client = HttpClient::new();

    // 1. Fetch workflow via TRS
    let tools_url = format!("{}/tools", cfg.services.trs_url.trim_end_matches('/'));
    let tools_val = client.get_json(&tools_url).await?;
    let tools = tools_val
        .as_array()
        .ok_or_else(|| anyhow::anyhow!("TRS /tools must return array"))?;
    let tool = &tools[0];
    let tool_id = tool
        .get("id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("Tool missing id field: {}", tool))?;

    let versions_url = format!(
        "{}/tools/{}/versions",
        cfg.services.trs_url.trim_end_matches('/'),
        tool_id
    );
    let versions_val = client.get_json(&versions_url).await?;
    let versions = versions_val
        .as_array()
        .ok_or_else(|| anyhow::anyhow!("TRS /tools/{{id}}/versions must return array"))?;
    let version = &versions[0];
    let version_id = version
        .get("id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("ToolVersion missing id: {}", version))?;

    // 2. Fetch input via DRS (object id expected to be tool-version specific)
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
    assert_eq!(
        drs_id, drs_object_id,
        "DRS id must propagate requested id"
    );

    // 3. Execute via WES using TRS URL and DRS object
    // Derive TRS registry host from the configured TRS_URL
    let trs_base = Url::parse(&cfg.services.trs_url)?;
    let host = trs_base
        .host_str()
        .ok_or_else(|| anyhow::anyhow!("TRS_URL missing host: {}", cfg.services.trs_url))?;
    let registry = if let Some(port) = trs_base.port() {
        format!("{}:{}", host, port)
    } else {
        host.to_string()
    };
    let trs_url = format!("trs://{}/{}/{}", registry, tool_id, version_id);
    let req = WesRunRequest {
        workflow_url: trs_url.clone(),
        workflow_type: "CWL".to_owned(),
        workflow_type_version: "v1.2".to_owned(),
        tags: None,
        workflow_params: serde_json::json!({
            "input_drs_uri": drs_obj.get("self_uri").cloned().unwrap_or(Value::String(format!("drs://{}", drs_id)))
        }),
    };
    let run_id = submit_wes_run(&client, &cfg.services.wes_url, &req).await?;

    // 4. Monitor TES (mock) – assume TES task id matches WES run id for this suite
    let tes_url = format!(
        "{}/tasks/{}",
        cfg.services.tes_url.trim_end_matches('/'),
        run_id
    );
    let tes_task = client.get_json(&tes_url).await?;
    let tes_task_id = tes_task
        .get("id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("TES task missing id: {}", tes_task))?;
    assert_eq!(
        tes_task_id, run_id,
        "TES task id must propagate WES run id"
    );

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
        "End-to-end pipeline must complete successfully"
    );

    // 5. Validate outputs (checksum)
    let outputs = fetch_wes_run_output(&client, &cfg.services.wes_url, &run_id).await?;
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
    let drs_out_id = drs_out_obj
        .get("id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("DRS output object missing id: {}", drs_out_obj))?;
    assert_eq!(
        drs_out_id, drs_output_id,
        "DRS output id must equal result_drs_id from WES outputs"
    );

    let expected_checksum_path = test_data_dir()
        .join("expected")
        .join("e2e")
        .join("result.txt.sha256");
    let expected_checksum = std::fs::read_to_string(&expected_checksum_path)?.trim().to_owned();

    // Download the result via DRS access_url and compute checksum from HTTP response
    let access_methods = drs_out_obj
        .get("access_methods")
        .and_then(|x| x.as_array())
        .ok_or_else(|| anyhow::anyhow!("DRS output object missing access_methods array: {}", drs_out_obj))?;
    let first = &access_methods[0];
    let access_url = first
        .get("access_url")
        .and_then(|a| a.get("url"))
        .and_then(|x| x.as_str())
        .ok_or_else(|| anyhow::anyhow!("access_methods[0].access_url.url missing: {}", drs_out_obj))?;

    let resp = client.inner().get(access_url).send().await?;
    if !resp.status().is_success() {
        anyhow::bail!(
            "Failed to download DRS output object for E2E checksum validation: {}",
            resp.status()
        );
    }
    let bytes = resp.bytes().await?;
    use sha2::Digest;
    let mut hasher = sha2::Sha256::new();
    hasher.update(&bytes);
    let actual_checksum = format!("{:x}", hasher.finalize());
    assert_eq!(
        actual_checksum, expected_checksum,
        "E2E pipeline result checksum mismatch"
    );

    // 6. Query Beacon to assert presence of test variant/sample
    let beacon_query_url = format!(
        "{}/query",
        cfg.services.beacon_url.trim_end_matches('/')
    );
    let beacon_resp = client
        .post_json(
            &beacon_query_url,
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
        .and_then(|v| v.as_bool())
        .ok_or_else(|| anyhow::anyhow!("Beacon response missing response.exists: {}", beacon_resp))?;
    assert!(
        exists,
        "Beacon must report existence for test variant after pipeline execution"
    );

    Ok(())
}

