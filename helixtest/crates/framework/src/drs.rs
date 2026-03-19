use anyhow::Result;
use common::config::TestConfig;
use common::http::HttpClient;
use common::report::{ComplianceLevel, ServiceKind, ServiceReport, TestCaseResult, TestCategory};
use serde_json::Value;
use tracing::info;

use crate::{Features, Mode};

pub async fn run_drs_checks(
    _mode: Mode,
    features: &Features,
    cfg: &TestConfig,
    client: &HttpClient,
) -> Result<ServiceReport> {
    let mut tests = Vec::new();

    tests.push(level0_reachable(cfg, client).await);
    tests.push(level1_basic_schema_and_fields(cfg, client).await);
    tests.push(level2_checksum_correctness(features, cfg, client).await);
    tests.push(level2_range_request(cfg, client).await);
    tests.push(level5_invalid_id_404(cfg, client).await);

    Ok(ServiceReport {
        service: ServiceKind::Drs,
        tests,
    })
}

async fn level0_reachable(cfg: &TestConfig, client: &HttpClient) -> TestCaseResult {
    let url = format!(
        "{}/objects/{}",
        cfg.services.drs_url.trim_end_matches('/'),
        "test-object-1"
    );
    let res = client.inner().get(&url).send().await;
    match res {
        Ok(resp) => TestCaseResult {
            name: "DRS object endpoint reachable".into(),
            level: ComplianceLevel::Level0,
            passed: resp.status().is_success() || resp.status().is_client_error(),
            error: (!resp.status().is_success() && !resp.status().is_client_error())
                .then(|| format!("Unexpected HTTP status: {}", resp.status())),
            category: TestCategory::Other,
            weight: 1.0,
        },
        Err(e) => TestCaseResult {
            name: "DRS object endpoint reachable".into(),
            level: ComplianceLevel::Level0,
            passed: false,
            error: Some(e.to_string()),
            category: TestCategory::Other,
            weight: 1.0,
        },
    }
}

async fn level1_basic_schema_and_fields(cfg: &TestConfig, client: &HttpClient) -> TestCaseResult {
    let url = format!(
        "{}/objects/{}",
        cfg.services.drs_url.trim_end_matches('/'),
        "test-object-1"
    );
    let res = client.get_json(&url).await;
    let result = res.and_then(|v| validate_basic_drs_object("test-object-1", &v));
    TestCaseResult {
        name: "DRS basic fields and access_methods".into(),
        level: ComplianceLevel::Level1,
        passed: result.is_ok(),
        error: result.err().map(|e| e.to_string()),
        category: TestCategory::Schema,
        weight: 1.0,
    }
}

fn validate_basic_drs_object(expected_id: &str, v: &Value) -> Result<()> {
    let id = v
        .get("id")
        .and_then(|x| x.as_str())
        .ok_or_else(|| anyhow::anyhow!("DRS object missing id: {}", v))?;
    if id != expected_id {
        anyhow::bail!("DRS id mismatch: expected {}, got {}", expected_id, id);
    }
    let _self_uri = v
        .get("self_uri")
        .and_then(|x| x.as_str())
        .ok_or_else(|| anyhow::anyhow!("DRS object missing self_uri: {}", v))?;
    let _name = v
        .get("name")
        .and_then(|x| x.as_str())
        .ok_or_else(|| anyhow::anyhow!("DRS object missing name: {}", v))?;

    let access_methods = v
        .get("access_methods")
        .and_then(|x| x.as_array())
        .ok_or_else(|| anyhow::anyhow!("DRS object missing access_methods array: {}", v))?;
    if access_methods.is_empty() {
        anyhow::bail!("DRS object must expose at least one access_method");
    }
    Ok(())
}

async fn level2_checksum_correctness(
    features: &Features,
    cfg: &TestConfig,
    client: &HttpClient,
) -> TestCaseResult {
    if !features.strict_drs_checksums {
        return TestCaseResult {
            name: "DRS checksum correctness".into(),
            level: ComplianceLevel::Level2,
            passed: true,
            error: Some(
                "DRS strict checksum check disabled (strict_drs_checksums=false in features)"
                    .into(),
            ),
            category: TestCategory::Checksum,
            weight: 1.0,
        };
    }

    let url = format!(
        "{}/objects/{}",
        cfg.services.drs_url.trim_end_matches('/'),
        "test-object-1"
    );
    let result = async {
        let v = client.get_json(&url).await?;
        let checksums = v
            .get("checksums")
            .and_then(|x| x.as_array())
            .ok_or_else(|| anyhow::anyhow!("DRS object missing checksums: {}", v))?;
        let checksum_entry = checksums
            .iter()
            .find(|c| {
                c.get("type")
                    .and_then(|t| t.as_str())
                    .map(|t| t.eq_ignore_ascii_case("sha256"))
                    .unwrap_or(false)
            })
            .ok_or_else(|| anyhow::anyhow!("No sha256 checksum entry in DRS object: {}", v))?;
        let expected_checksum = checksum_entry
            .get("checksum")
            .and_then(|x| x.as_str())
            .ok_or_else(|| anyhow::anyhow!("sha256 checksum entry missing checksum field"))?;

        // Download the object bytes via access_url and compute checksum from HTTP response
        let access_methods = v
            .get("access_methods")
            .and_then(|x| x.as_array())
            .ok_or_else(|| anyhow::anyhow!("DRS object missing access_methods array: {}", v))?;
        let first = &access_methods[0];
        let access_url = first
            .get("access_url")
            .and_then(|a| a.get("url"))
            .and_then(|x| x.as_str())
            .ok_or_else(|| anyhow::anyhow!("access_methods[0].access_url.url missing: {}", v))?;

        let resp = client.inner().get(access_url).send().await?;
        if !resp.status().is_success() {
            anyhow::bail!(
                "Failed to download DRS object for checksum validation: {}",
                resp.status()
            );
        }
        let bytes = resp.bytes().await?;

        let mut hasher = sha2::Sha256::new();
        use sha2::Digest;
        hasher.update(&bytes);
        let actual = format!("{:x}", hasher.finalize());
        info!(expected = %expected_checksum, actual = %actual, "DRS checksum comparison from HTTP download");
        if !actual.eq_ignore_ascii_case(expected_checksum) {
            anyhow::bail!(
                "DRS checksum mismatch for test-object-1: expected {}, got {}",
                expected_checksum,
                actual
            );
        }
        Ok::<(), anyhow::Error>(())
    }
    .await;

    TestCaseResult {
        name: "DRS checksum correctness".into(),
        level: ComplianceLevel::Level2,
        passed: result.is_ok(),
        error: result.err().map(|e| e.to_string()),
        category: TestCategory::Checksum,
        weight: 1.0,
    }
}

async fn level2_range_request(cfg: &TestConfig, client: &HttpClient) -> TestCaseResult {
    let url = format!(
        "{}/objects/{}",
        cfg.services.drs_url.trim_end_matches('/'),
        "test-object-1"
    );
    let result = async {
        let v = client.get_json(&url).await?;
        let access_methods = v
            .get("access_methods")
            .and_then(|x| x.as_array())
            .ok_or_else(|| anyhow::anyhow!("DRS object missing access_methods array: {}", v))?;
        let first = &access_methods[0];
        let url = first
            .get("access_url")
            .and_then(|a| a.get("url"))
            .and_then(|x| x.as_str())
            .ok_or_else(|| anyhow::anyhow!("access_methods[0].access_url.url missing: {}", v))?;

        let resp = client
            .inner()
            .get(url)
            .header("Range", "bytes=0-1023")
            .send()
            .await?;
        if resp.status().as_u16() != 206 {
            anyhow::bail!(
                "Expected 206 Partial Content for range request, got {}",
                resp.status()
            );
        }
        // Validate Content-Range header
        let headers = resp.headers().clone();
        let content_range = headers
            .get("Content-Range")
            .and_then(|v| v.to_str().ok())
            .ok_or_else(|| anyhow::anyhow!("Missing Content-Range header on 206 response"))?;
        // Expected pattern: "bytes START-END/TOTAL"
        let parts: Vec<&str> = content_range.split_whitespace().collect();
        if parts.len() != 2 || parts[0] != "bytes" {
            anyhow::bail!("Invalid Content-Range format: {}", content_range);
        }
        let range_part = parts[1];
        let range_and_total: Vec<&str> = range_part.split('/').collect();
        if range_and_total.len() != 2 {
            anyhow::bail!("Invalid Content-Range range/total: {}", content_range);
        }
        let range = range_and_total[0];
        let range_bounds: Vec<&str> = range.split('-').collect();
        if range_bounds.len() != 2 {
            anyhow::bail!("Invalid Content-Range bounds: {}", content_range);
        }
        let start: u64 = range_bounds[0]
            .parse()
            .map_err(|_| anyhow::anyhow!("Invalid start in Content-Range: {}", content_range))?;
        let end: u64 = range_bounds[1]
            .parse()
            .map_err(|_| anyhow::anyhow!("Invalid end in Content-Range: {}", content_range))?;
        if start != 0 {
            anyhow::bail!(
                "Content-Range start must be 0 for request bytes=0-1023, got {} in {}",
                start,
                content_range
            );
        }
        if end < start || end > 1023 {
            anyhow::bail!(
                "Content-Range end must be between 0 and 1023, got {} in {}",
                end,
                content_range
            );
        }

        let bytes = resp.bytes().await?;
        let body: Vec<u8> = bytes.iter().take(2049).cloned().collect();
        if body.is_empty() {
            anyhow::bail!("Range request returned empty body");
        }
        if body.len() > 2048 {
            anyhow::bail!(
                "Range request returned unexpectedly large body: {} bytes",
                body.len()
            );
        }

        Ok::<(), anyhow::Error>(())
    }
    .await;

    TestCaseResult {
        name: "DRS HTTP Range support".into(),
        level: ComplianceLevel::Level2,
        passed: result.is_ok(),
        error: result.err().map(|e| e.to_string()),
        category: TestCategory::Interoperability,
        weight: 1.0,
    }
}

async fn level5_invalid_id_404(cfg: &TestConfig, client: &HttpClient) -> TestCaseResult {
    let url = format!(
        "{}/objects/{}",
        cfg.services.drs_url.trim_end_matches('/'),
        "nonexistent-object-id-for-conformance"
    );
    let res = client.inner().get(&url).send().await;
    let result = res.map_err(anyhow::Error::from).and_then(|resp| {
        if resp.status().as_u16() == 404 {
            Ok(())
        } else {
            Err(anyhow::anyhow!(
                "Expected 404 for invalid DRS id, got {}",
                resp.status()
            ))
        }
    });

    TestCaseResult {
        name: "DRS invalid object id returns 404".into(),
        level: ComplianceLevel::Level5,
        passed: result.is_ok(),
        error: result.err().map(|e| e.to_string()),
        category: TestCategory::Robustness,
        weight: 1.0,
    }
}
