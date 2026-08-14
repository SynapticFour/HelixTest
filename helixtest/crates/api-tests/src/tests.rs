use anyhow::Result;
use common::config::TestConfig;
use common::ga4gh_schemas::{
    validate_trs_tool, validate_trs_tool_version, validate_wes_service_info,
};
use common::http::HttpClient;
use common::schemas::assert_required_string_field;

#[tokio::test]
async fn wes_service_info_schema_and_fields() -> Result<()> {
    let cfg = TestConfig::from_env_or_file()?;
    let client = HttpClient::new();
    let url = format!(
        "{}/service-info",
        cfg.services.wes_url.trim_end_matches('/')
    );
    let v = client.get_json(&url).await?;

    validate_wes_service_info(&v)?;

    let versions = v
        .get("supported_wes_versions")
        .and_then(|x| x.as_array())
        .ok_or_else(|| anyhow::anyhow!("supported_wes_versions must be an array"))?;
    let has_1_0 = versions
        .iter()
        .filter_map(|v| v.as_str())
        .any(|s| s == "1.0" || s == "1.1");
    assert!(
        has_1_0,
        "supported_wes_versions must contain at least 1.0 or 1.1, got {:?}",
        versions
    );

    Ok(())
}

#[tokio::test]
async fn drs_get_object_required_fields() -> Result<()> {
    let cfg = TestConfig::from_env_or_file()?;
    let client = HttpClient::new();
    let object_id = "test-object-1";
    let url = format!(
        "{}/objects/{}",
        cfg.services.drs_url.trim_end_matches('/'),
        object_id
    );
    let v = client.get_json(&url).await?;

    let id = assert_required_string_field(&v, "id")?;
    assert_eq!(
        id, object_id,
        "DRS object id must match requested id (propagation check)"
    );
    let _self_uri = assert_required_string_field(&v, "self_uri")?;
    let _name = assert_required_string_field(&v, "name")?;

    let access_methods = v
        .get("access_methods")
        .and_then(|x| x.as_array())
        .ok_or_else(|| anyhow::anyhow!("access_methods must be an array"))?;
    assert!(
        !access_methods.is_empty(),
        "DRS object must expose at least one access_method"
    );

    Ok(())
}

#[tokio::test]
async fn trs_tools_and_versions_contract() -> Result<()> {
    let cfg = TestConfig::from_env_or_file()?;
    let client = HttpClient::new();
    let tools_url = format!("{}/tools", cfg.services.trs_url.trim_end_matches('/'));
    let tools_val = client.get_json(&tools_url).await?;
    let tools = tools_val
        .as_array()
        .ok_or_else(|| anyhow::anyhow!("TRS /tools must return array"))?;
    assert!(
        !tools.is_empty(),
        "TRS must expose at least one tool for testing"
    );

    let first_tool = &tools[0];
    validate_trs_tool(first_tool)?;
    let tool_id = assert_required_string_field(first_tool, "id")?;

    let versions_url = format!(
        "{}/tools/{}/versions",
        cfg.services.trs_url.trim_end_matches('/'),
        tool_id
    );
    let versions_val = client.get_json(&versions_url).await?;
    let versions = versions_val
        .as_array()
        .ok_or_else(|| anyhow::anyhow!("TRS /tools/{{id}}/versions must return array"))?;
    assert!(
        !versions.is_empty(),
        "TRS tool must expose at least one version"
    );
    validate_trs_tool_version(&versions[0])?;

    Ok(())
}
