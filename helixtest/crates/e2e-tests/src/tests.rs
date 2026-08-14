use anyhow::Result;
use common::config::TestConfig;
use common::http::HttpClient;
use common::util::{sha256_bytes, test_data_dir};
use common::workflow::{
    fetch_wes_run_output, poll_wes_run_until_terminal, submit_wes_run, WesRunRequest,
};
use serde_json::Value;
use std::time::Duration;
use url::Url;

fn preferred_drs_input_uri(drs_obj: &Value, drs_id: &str) -> String {
    // E2E policy: use DRS URI by default. If service returns a self_uri, accept it only
    // when it is already a drs:// URI; otherwise fall back to canonical drs://<id>.
    if let Some(self_uri) = drs_obj.get("self_uri").and_then(|v| v.as_str()) {
        if self_uri.starts_with("drs://") {
            return self_uri.to_string();
        }
    }
    format!("drs://{}", drs_id)
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
    assert_eq!(drs_id, drs_object_id, "DRS id must propagate requested id");

    // 3. Execute via WES using TRS URL and DRS object
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
            "input_drs_uri": preferred_drs_input_uri(&drs_obj, drs_id)
        }),
    };
    let run_id = submit_wes_run(&client, &cfg.services.wes_url, &req).await?;

    // TES is independent of WES run_id (backends do not share IDs). Probe reachability only.
    let tes_url = format!("{}/tasks", cfg.services.tes_url.trim_end_matches('/'));
    let tes_resp = client.inner().get(&tes_url).send().await?;
    if !tes_resp.status().is_success() && tes_resp.status().as_u16() != 401 {
        anyhow::bail!("TES /tasks not reachable: {}", tes_resp.status());
    }

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

    let expected_checksum_path = test_data_dir()?
        .join("expected")
        .join("e2e")
        .join("result.txt.sha256");
    let expected_checksum = std::fs::read_to_string(&expected_checksum_path)
        .map_err(|_| {
            anyhow::anyhow!(
                "E2E golden checksum missing at {} — add the expected SHA-256; refusing to treat actual as expected",
                expected_checksum_path.display()
            )
        })?
        .trim()
        .to_owned();

    let access_methods = drs_out_obj
        .get("access_methods")
        .and_then(|x| x.as_array())
        .ok_or_else(|| {
            anyhow::anyhow!(
                "DRS output object missing access_methods array: {}",
                drs_out_obj
            )
        })?;
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
        anyhow::bail!(
            "Failed to download DRS output object for E2E checksum validation: {}",
            resp.status()
        );
    }
    let bytes = resp.bytes().await?;
    let actual_checksum = sha256_bytes(&bytes);
    assert!(
        actual_checksum.eq_ignore_ascii_case(&expected_checksum),
        "E2E pipeline result checksum mismatch: expected {}, got {}",
        expected_checksum,
        actual_checksum
    );

    // 6. Query Beacon to assert presence of test variant/sample
    let beacon_query_url = format!("{}/query", cfg.services.beacon_url.trim_end_matches('/'));
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
        .ok_or_else(|| {
            anyhow::anyhow!("Beacon response missing response.exists: {}", beacon_resp)
        })?;
    assert!(
        exists,
        "Beacon must report existence for test variant after pipeline execution"
    );

    Ok(())
}
