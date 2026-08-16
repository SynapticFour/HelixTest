// SPDX-License-Identifier: Apache-2.0
use anyhow::Result;
use common::config::TestConfig;
use common::ga4gh_schemas;
use common::http::HttpClient;
use common::report::{ComplianceLevel, ServiceKind, ServiceReport, TestCaseResult, TestCategory};
use serde_json::json;
use tracing::info;

use crate::{level0_http, Features, Mode};

fn beacon_exists(v: &serde_json::Value) -> Result<bool> {
    v.pointer("/responseSummary/exists")
        .or_else(|| v.pointer("/response/exists"))
        .and_then(|x| x.as_bool())
        .ok_or_else(|| {
            anyhow::anyhow!(
                "Beacon response missing responseSummary.exists (and response.exists): {v}"
            )
        })
}

pub async fn run_beacon_checks(
    _mode: Mode,
    features: &Features,
    cfg: &TestConfig,
    client: &HttpClient,
) -> Result<ServiceReport> {
    let mut tests = Vec::new();
    tests.push(level0_reachable(cfg, client).await);
    tests.push(level1_schema(cfg, client).await);
    tests.push(level2_known_variant_exists(features, cfg, client).await);
    tests.push(level2_negative_variant_not_exists(features, cfg, client).await);

    Ok(ServiceReport {
        service: ServiceKind::Beacon,
        tests,
    })
}

async fn level0_reachable(cfg: &TestConfig, client: &HttpClient) -> TestCaseResult {
    let url = format!("{}/query", cfg.services.beacon_url.trim_end_matches('/'));
    let res = client
        .inner()
        .post(&url)
        .json(&json!({
            "meta": { "apiVersion": "v2.0.0" },
            "query": { "requestParameters": {} }
        }))
        .send()
        .await;
    level0_http("Beacon /query reachable", res)
}

async fn level1_schema(cfg: &TestConfig, client: &HttpClient) -> TestCaseResult {
    let res = async {
        let url = format!("{}/query", cfg.services.beacon_url.trim_end_matches('/'));
        let v = client
            .post_json(
                &url,
                &json!({
                    "meta": { "apiVersion": "v2.0.0" },
                    "query": { "requestParameters": {} }
                }),
            )
            .await?;
        ga4gh_schemas::validate_beacon_boolean_response(&v)?;
        Ok::<(), anyhow::Error>(())
    }
    .await;

    TestCaseResult::from_outcome(
        "Beacon boolean response (official schema)",
        ComplianceLevel::Level1,
        TestCategory::Schema,
        res,
    )
}

async fn level2_known_variant_exists(
    features: &Features,
    cfg: &TestConfig,
    client: &HttpClient,
) -> TestCaseResult {
    if !features.supports_beacon_v2 {
        return TestCaseResult::skip(
            "Beacon known variant exists",
            ComplianceLevel::Level2,
            TestCategory::Interoperability,
            "supports_beacon_v2=false in features",
        );
    }

    let res = async {
        let url = format!("{}/query", cfg.services.beacon_url.trim_end_matches('/'));
        let v = client
            .post_json(
                &url,
                &json!({
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
        let exists = beacon_exists(&v)?;
        info!(
            referenceName = "1",
            start = 1000,
            referenceBases = "A",
            alternateBases = "T",
            %exists,
            "Beacon positive test variant query"
        );
        if !exists {
            anyhow::bail!(
                "Beacon expected to report existence for known test variant, but exists=false"
            );
        }
        Ok::<(), anyhow::Error>(())
    }
    .await;

    TestCaseResult::from_outcome(
        "Beacon known variant exists",
        ComplianceLevel::Level2,
        TestCategory::Interoperability,
        res,
    )
}

async fn level2_negative_variant_not_exists(
    features: &Features,
    cfg: &TestConfig,
    client: &HttpClient,
) -> TestCaseResult {
    if !features.supports_beacon_v2 {
        return TestCaseResult::skip(
            "Beacon negative variant not exists",
            ComplianceLevel::Level2,
            TestCategory::Interoperability,
            "supports_beacon_v2=false in features",
        );
    }

    let res = async {
        let url = format!("{}/query", cfg.services.beacon_url.trim_end_matches('/'));
        let v = client
            .post_json(
                &url,
                &json!({
                    "meta": { "apiVersion": "v2.0.0" },
                    "query": {
                        "requestParameters": {
                            "referenceName": "1",
                            "start": 999999999,
                            "referenceBases": "C",
                            "alternateBases": "G"
                        }
                    }
                }),
            )
            .await?;
        let exists = beacon_exists(&v)?;
        info!(
            referenceName = "1",
            start = 999999999i64,
            referenceBases = "C",
            alternateBases = "G",
            %exists,
            "Beacon negative test variant query"
        );
        if exists {
            anyhow::bail!(
                "Beacon expected to report non-existence for negative test variant, but exists=true"
            );
        }
        Ok::<(), anyhow::Error>(())
    }
    .await;

    TestCaseResult::from_outcome(
        "Beacon negative variant not exists",
        ComplianceLevel::Level2,
        TestCategory::Interoperability,
        res,
    )
}
