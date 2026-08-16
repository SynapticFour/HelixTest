// SPDX-License-Identifier: Apache-2.0
use anyhow::Result;
use common::config::TestConfig;
use common::http::HttpClient;
use common::report::{ComplianceLevel, ServiceKind, ServiceReport, TestCaseResult, TestCategory};
use common::util::sha256_bytes;
use futures::StreamExt;
use serde_json::Value;
use tracing::info;

use crate::{level0_http, Features, Mode};

const RANGE_BODY_LIMIT: usize = 2048;

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
    level0_http(
        "DRS object endpoint reachable",
        client.inner().get(&url).send().await,
    )
}

async fn level1_basic_schema_and_fields(cfg: &TestConfig, client: &HttpClient) -> TestCaseResult {
    let url = format!(
        "{}/objects/{}",
        cfg.services.drs_url.trim_end_matches('/'),
        "test-object-1"
    );
    let res = client
        .get_json(&url)
        .await
        .and_then(|v| {
            common::ga4gh_schemas::validate_drs_object(&v)?;
            validate_basic_drs_object("test-object-1", &v)?;
            Ok(v)
        })
        .map(|_| ());
    TestCaseResult::from_outcome(
        "DRS DrsObject OpenAPI + access_methods",
        ComplianceLevel::Level1,
        TestCategory::Schema,
        res,
    )
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

fn first_access_url(v: &Value) -> Result<&str> {
    let access_methods = v
        .get("access_methods")
        .and_then(|x| x.as_array())
        .ok_or_else(|| anyhow::anyhow!("DRS object missing access_methods array: {}", v))?;
    access_methods
        .first()
        .and_then(|first| first.get("access_url"))
        .and_then(|a| a.get("url"))
        .and_then(|x| x.as_str())
        .ok_or_else(|| anyhow::anyhow!("access_methods[0].access_url.url missing: {}", v))
}

async fn level2_checksum_correctness(
    features: &Features,
    cfg: &TestConfig,
    client: &HttpClient,
) -> TestCaseResult {
    if !features.strict_drs_checksums {
        return TestCaseResult::skip(
            "DRS checksum correctness",
            ComplianceLevel::Level2,
            TestCategory::Checksum,
            "strict_drs_checksums=false in features",
        );
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

        let access_url = first_access_url(&v)?;
        let resp = client.inner().get(access_url).send().await?;
        if !resp.status().is_success() {
            anyhow::bail!(
                "Failed to download DRS object for checksum validation: {}",
                resp.status()
            );
        }
        let bytes = resp.bytes().await?;
        let actual = sha256_bytes(&bytes);
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

    TestCaseResult::from_outcome(
        "DRS checksum correctness",
        ComplianceLevel::Level2,
        TestCategory::Checksum,
        result,
    )
}

async fn read_body_capped(resp: reqwest::Response, limit: usize) -> Result<Vec<u8>> {
    let mut stream = resp.bytes_stream();
    let mut body = Vec::with_capacity(limit.min(1024));
    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        let room = limit.saturating_add(1).saturating_sub(body.len());
        if chunk.len() > room {
            anyhow::bail!(
                "Range request returned unexpectedly large body (over {} bytes)",
                limit
            );
        }
        body.extend_from_slice(&chunk);
        if body.len() > limit {
            anyhow::bail!(
                "Range request returned unexpectedly large body: {} bytes",
                body.len()
            );
        }
    }
    Ok(body)
}

fn parse_content_range(content_range: &str) -> Result<(u64, u64)> {
    let (unit, rest) = content_range
        .split_once(char::is_whitespace)
        .ok_or_else(|| anyhow::anyhow!("Invalid Content-Range format: {}", content_range))?;
    if unit != "bytes" {
        anyhow::bail!("Invalid Content-Range format: {}", content_range);
    }
    let (range, _total) = rest
        .split_once('/')
        .ok_or_else(|| anyhow::anyhow!("Invalid Content-Range range/total: {}", content_range))?;
    let (start_s, end_s) = range
        .split_once('-')
        .ok_or_else(|| anyhow::anyhow!("Invalid Content-Range bounds: {}", content_range))?;
    let start: u64 = start_s
        .parse()
        .map_err(|_| anyhow::anyhow!("Invalid start in Content-Range: {}", content_range))?;
    let end: u64 = end_s
        .parse()
        .map_err(|_| anyhow::anyhow!("Invalid end in Content-Range: {}", content_range))?;
    Ok((start, end))
}

async fn level2_range_request(cfg: &TestConfig, client: &HttpClient) -> TestCaseResult {
    let url = format!(
        "{}/objects/{}",
        cfg.services.drs_url.trim_end_matches('/'),
        "test-object-1"
    );
    let result = async {
        let v = client.get_json(&url).await?;
        let access_url = first_access_url(&v)?;

        let resp = client
            .inner()
            .get(access_url)
            .header("Range", "bytes=0-1023")
            .send()
            .await?;
        if resp.status().as_u16() != 206 {
            anyhow::bail!(
                "Expected 206 Partial Content for range request, got {}",
                resp.status()
            );
        }
        let content_range = resp
            .headers()
            .get("Content-Range")
            .and_then(|v| v.to_str().ok())
            .ok_or_else(|| anyhow::anyhow!("Missing Content-Range header on 206 response"))?
            .to_string();
        let (start, end) = parse_content_range(&content_range)?;
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

        let body = read_body_capped(resp, RANGE_BODY_LIMIT).await?;
        if body.is_empty() {
            anyhow::bail!("Range request returned empty body");
        }

        Ok::<(), anyhow::Error>(())
    }
    .await;

    TestCaseResult::from_outcome(
        "DRS HTTP Range support",
        ComplianceLevel::Level2,
        TestCategory::Interoperability,
        result,
    )
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

    TestCaseResult::from_outcome(
        "DRS invalid object id returns 404",
        ComplianceLevel::Level5,
        TestCategory::Robustness,
        result,
    )
}
