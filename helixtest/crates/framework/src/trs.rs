use anyhow::Result;
use common::config::TestConfig;
use common::http::HttpClient;
use common::report::{ComplianceLevel, ServiceKind, ServiceReport, TestCaseResult, TestCategory};

use crate::{Features, Mode};

pub async fn run_trs_checks(
    _mode: Mode,
    _features: &Features,
    cfg: &TestConfig,
    client: &HttpClient,
) -> Result<ServiceReport> {
    let mut tests = Vec::new();
    tests.push(level0_reachable(cfg, client).await);
    tests.push(level1_schema_and_fields(cfg, client).await);
    tests.push(level2_descriptor_retrieval(cfg, client).await);

    Ok(ServiceReport {
        service: ServiceKind::Trs,
        tests,
    })
}

async fn level0_reachable(cfg: &TestConfig, client: &HttpClient) -> TestCaseResult {
    let url = format!("{}/tools", cfg.services.trs_url.trim_end_matches('/'));
    let res = client.inner().get(&url).send().await;
    match res {
        Ok(resp) => TestCaseResult {
            name: "TRS /tools reachable".into(),
            level: ComplianceLevel::Level0,
            passed: resp.status().is_success() || resp.status().is_client_error(),
            error: (!resp.status().is_success() && !resp.status().is_client_error())
                .then(|| format!("Unexpected HTTP status: {}", resp.status())),
            category: TestCategory::Other,
            weight: 1.0,
        },
        Err(e) => TestCaseResult {
            name: "TRS /tools reachable".into(),
            level: ComplianceLevel::Level0,
            passed: false,
            error: Some(e.to_string()),
            category: TestCategory::Other,
            weight: 1.0,
        },
    }
}

async fn level1_schema_and_fields(cfg: &TestConfig, client: &HttpClient) -> TestCaseResult {
    let res = async {
        let tools_url = format!("{}/tools", cfg.services.trs_url.trim_end_matches('/'));
        let tools_val = client.get_json(&tools_url).await?;
        let tools = tools_val
            .as_array()
            .ok_or_else(|| anyhow::anyhow!("TRS /tools must return array"))?;
        if tools.is_empty() {
            anyhow::bail!("TRS must expose at least one tool");
        }
        common::ga4gh_schemas::validate_trs_tool(&tools[0])?;

        let tool_id = tools[0]
            .get("id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Tool missing id: {}", tools[0]))?;

        let versions_url = format!(
            "{}/tools/{}/versions",
            cfg.services.trs_url.trim_end_matches('/'),
            tool_id
        );
        let versions_val = client.get_json(&versions_url).await?;
        let versions = versions_val
            .as_array()
            .ok_or_else(|| anyhow::anyhow!("TRS /tools/{{id}}/versions must return array"))?;
        if versions.is_empty() {
            anyhow::bail!("TRS tool must expose at least one version");
        }
        common::ga4gh_schemas::validate_trs_tool_version(&versions[0])?;
        Ok::<(), anyhow::Error>(())
    }
    .await;

    TestCaseResult {
        name: "TRS tools and versions schema".into(),
        level: ComplianceLevel::Level1,
        passed: res.is_ok(),
        error: res.err().map(|e| e.to_string()),
        category: TestCategory::Schema,
        weight: 1.0,
    }
}

async fn level2_descriptor_retrieval(cfg: &TestConfig, client: &HttpClient) -> TestCaseResult {
    let res = async {
        let tools_url = format!("{}/tools", cfg.services.trs_url.trim_end_matches('/'));
        let tools_val = client.get_json(&tools_url).await?;
        let tools = tools_val
            .as_array()
            .ok_or_else(|| anyhow::anyhow!("TRS /tools must return array"))?;
        let first_tool = &tools[0];
        let tool_id = first_tool
            .get("id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Tool missing id: {}", first_tool))?;

        let versions_url = format!(
            "{}/tools/{}/versions",
            cfg.services.trs_url.trim_end_matches('/'),
            tool_id
        );
        let versions_val = client.get_json(&versions_url).await?;
        let versions = versions_val
            .as_array()
            .ok_or_else(|| anyhow::anyhow!("TRS /tools/{{id}}/versions must return array"))?;
        let first_version = &versions[0];
        let version_id = first_version
            .get("id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("ToolVersion missing id: {}", first_version))?;

        // Attempt to fetch a CWL descriptor; implementations may support other types too.
        let desc_url = format!(
            "{}/tools/{}/versions/{}/PLAIN_CWL/descriptor",
            cfg.services.trs_url.trim_end_matches('/'),
            tool_id,
            version_id
        );
        let resp = client.inner().get(&desc_url).send().await?;
        if !resp.status().is_success() {
            anyhow::bail!(
                "Descriptor retrieval failed for tool {} version {}: {}",
                tool_id,
                version_id,
                resp.status()
            );
        }
        let text = resp.text().await?;
        if text.trim().is_empty() {
            anyhow::bail!("Descriptor for tool {} version {} is empty", tool_id, version_id);
        }
        Ok::<(), anyhow::Error>(())
    }
    .await;

    TestCaseResult {
        name: "TRS descriptor retrieval".into(),
        level: ComplianceLevel::Level2,
        passed: res.is_ok(),
        error: res.err().map(|e| e.to_string()),
        category: TestCategory::Interoperability,
        weight: 1.0,
    }
}
