// SPDX-License-Identifier: Apache-2.0
use anyhow::Result;
use common::config::TestConfig;
use common::http::HttpClient;
use common::report::{ComplianceLevel, ServiceKind, ServiceReport, TestCaseResult, TestCategory};
use common::spec_source::{SpecCompileResult, SpecSource};
use common::util::sha256_bytes;
use futures::StreamExt;
use serde_json::Value;
use tracing::info;

use crate::{level0_http, Features, Mode};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

const RANGE_BODY_LIMIT: usize = 2048;
/// Cap on checksum downloads. Not a genomic-object fetch.
pub const CHECKSUM_BODY_LIMIT: usize = 2 * 1024 * 1024;

/// SHA-256 of the DRS checker sources compiled into this crate (`build.rs`).
/// Not VERSIONS.lock. Not a git tag.
pub fn executed_checker_source_sha256() -> &'static str {
    env!("HELIXTEST_DRS_CHECKER_SOURCE_SHA256")
}

/// Executed DRS checker identity. Helix must report this, not a lockfile SHA.
pub fn executed_checker_id() -> String {
    format!("helixtest-drs:{}", executed_checker_source_sha256())
}

/// Default DRS object id for the Helix/HelixTest in-process mock catalog.
pub const DEFAULT_DRS_OBJECT_ID: &str = "test-object-1";

/// Stable skip token. Helix maps this to fixture-unavailable attribution, not
/// target non-conformance. A 404 on the configured object is missing test input.
pub const FIXTURE_UNAVAILABLE: &str = "fixture_unavailable";

/// HelixTest name for the versioned-only OpenAPI check (pinned SpecSource, no extras).
pub const DRS_OPENAPI_SPECSOURCE_CHECK: &str = "DRS DrsObject OpenAPI SpecSource";

/// Incremented on every entry to [`run_drs_checks_with_spec`]. Always compiled so
/// Helix (path dep, non-test cfg) can prove a corrupt pack never reached this
/// function. Not a second checker.
static WITH_SPEC_CALLS: AtomicUsize = AtomicUsize::new(0);

/// Test-only: forge `schema_document_sha256` returned by the next
/// [`run_drs_checks_with_spec`]. Default off. Production never sets this.
/// Always compiled so Helix integration tests can exercise the real join.
static LIE_SPEC_DOCUMENT_HASH: AtomicBool = AtomicBool::new(false);

/// Target-owned DRS test input. Not a GA4GH requirement. Not implementation identity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DrsTestFixture {
    pub object_id: String,
    /// Independently known sha256 of the blob bytes. When set, checksum does not
    /// take expected digest from the GetObject JSON under test.
    pub expected_sha256: Option<String>,
}

impl Default for DrsTestFixture {
    fn default() -> Self {
        Self {
            object_id: DEFAULT_DRS_OBJECT_ID.to_string(),
            expected_sha256: None,
        }
    }
}

impl DrsTestFixture {
    /// Deterministic unknown id derived from the positive fixture. Not random.
    pub fn unknown_object_id(&self) -> String {
        unknown_object_id_for(&self.object_id)
    }
}

pub fn unknown_object_id_for(object_id: &str) -> String {
    let digest = sha256_bytes(format!("helix.unknown\n{object_id}").as_bytes());
    let id = format!("helix.unknown.{:.32}", digest);
    if id == object_id {
        format!("{id}.absent")
    } else {
        id
    }
}

pub fn with_spec_calls() -> usize {
    WITH_SPEC_CALLS.load(Ordering::SeqCst)
}

pub fn reset_with_spec_calls() {
    WITH_SPEC_CALLS.store(0, Ordering::SeqCst);
    LIE_SPEC_DOCUMENT_HASH.store(false, Ordering::SeqCst);
}

/// Next [`run_drs_checks_with_spec`] returns a forged `schema_document_sha256`.
/// Helix uses this to prove identity mismatch discards the join. Not production.
pub fn set_lie_spec_document_hash(lie: bool) {
    LIE_SPEC_DOCUMENT_HASH.store(lie, Ordering::SeqCst);
}

pub async fn run_drs_checks(
    mode: Mode,
    features: &Features,
    cfg: &TestConfig,
    client: &HttpClient,
) -> Result<ServiceReport> {
    run_drs_checks_with_fixture(mode, features, cfg, client, &DrsTestFixture::default()).await
}

pub async fn run_drs_checks_with_fixture(
    _mode: Mode,
    features: &Features,
    cfg: &TestConfig,
    client: &HttpClient,
    fixture: &DrsTestFixture,
) -> Result<ServiceReport> {
    let mut tests = Vec::new();

    tests.push(level0_reachable(cfg, client, fixture).await);
    tests.push(level1_basic_schema_and_fields(cfg, client, fixture).await);
    tests.push(level2_checksum_correctness(features, cfg, client, fixture).await);
    tests.push(level2_range_request(cfg, client, fixture).await);
    tests.push(level5_invalid_id_404(cfg, client, fixture).await);

    Ok(ServiceReport {
        service: ServiceKind::Drs,
        tests,
    })
}

/// Versioned path: compile `spec` first (no bundled OpenAPI), then run the same HTTP checks.
pub async fn run_drs_checks_with_spec(
    mode: Mode,
    features: &Features,
    cfg: &TestConfig,
    client: &HttpClient,
    spec: &SpecSource,
) -> Result<(ServiceReport, SpecCompileResult)> {
    run_drs_checks_with_spec_and_fixture(
        mode,
        features,
        cfg,
        client,
        spec,
        &DrsTestFixture::default(),
    )
    .await
}

pub async fn run_drs_checks_with_spec_and_fixture(
    _mode: Mode,
    features: &Features,
    cfg: &TestConfig,
    client: &HttpClient,
    spec: &SpecSource,
    fixture: &DrsTestFixture,
) -> Result<(ServiceReport, SpecCompileResult)> {
    WITH_SPEC_CALLS.fetch_add(1, Ordering::SeqCst);
    let mut compile = common::spec_source::compile_identity(spec)?;
    if LIE_SPEC_DOCUMENT_HASH.swap(false, Ordering::SeqCst) {
        compile.schema_document_sha256 = "0".repeat(64);
    }
    let mut tests = Vec::new();

    tests.push(level0_reachable(cfg, client, fixture).await);
    tests.push(level1_openapi_specsource(cfg, client, spec, fixture).await);
    tests.push(level1_basic_schema_and_fields_with_spec(cfg, client, spec, fixture).await);
    tests.push(level2_checksum_correctness(features, cfg, client, fixture).await);
    tests.push(level2_range_request(cfg, client, fixture).await);
    tests.push(level5_invalid_id_404(cfg, client, fixture).await);

    Ok((
        ServiceReport {
            service: ServiceKind::Drs,
            tests,
        },
        compile,
    ))
}

fn object_url(cfg: &TestConfig, object_id: &str) -> String {
    format!(
        "{}/objects/{}",
        cfg.services.drs_url.trim_end_matches('/'),
        object_id
    )
}

fn skip_fixture(
    name: &str,
    level: ComplianceLevel,
    category: TestCategory,
    detail: impl std::fmt::Display,
) -> TestCaseResult {
    TestCaseResult::skip(
        name,
        level,
        category,
        format!("{FIXTURE_UNAVAILABLE}: {detail}"),
    )
}

enum ObjectGet {
    Json(Value),
    NotFound { status: u16 },
}

async fn get_object(client: &HttpClient, url: &str) -> Result<ObjectGet> {
    let resp = client.inner().get(url).send().await?;
    let status = resp.status().as_u16();
    if status == 404 {
        return Ok(ObjectGet::NotFound { status });
    }
    if !resp.status().is_success() {
        let text = resp.text().await.unwrap_or_default();
        anyhow::bail!("GET {url} failed with HTTP {status}: {text}");
    }
    let text = resp.text().await?;
    let value: Value = serde_json::from_str(&text)?;
    Ok(ObjectGet::Json(value))
}

async fn level0_reachable(
    cfg: &TestConfig,
    client: &HttpClient,
    fixture: &DrsTestFixture,
) -> TestCaseResult {
    let url = object_url(cfg, &fixture.object_id);
    let send = client.inner().get(&url).send().await;
    match &send {
        Ok(resp) if resp.status().as_u16() == 404 => {
            return skip_fixture(
                "DRS object endpoint reachable",
                ComplianceLevel::Level0,
                TestCategory::Other,
                format!("GET {url} returned 404 (object_id={})", fixture.object_id),
            );
        }
        _ => {}
    }
    level0_http("DRS object endpoint reachable", send)
}

async fn level1_basic_schema_and_fields(
    cfg: &TestConfig,
    client: &HttpClient,
    fixture: &DrsTestFixture,
) -> TestCaseResult {
    let url = object_url(cfg, &fixture.object_id);
    match get_object(client, &url).await {
        Ok(ObjectGet::NotFound { status }) => skip_fixture(
            "DRS DrsObject OpenAPI + access_methods",
            ComplianceLevel::Level1,
            TestCategory::Schema,
            format!(
                "GET {url} returned {status} (object_id={})",
                fixture.object_id
            ),
        ),
        Ok(ObjectGet::Json(v)) => {
            let res: Result<()> = (|| {
                common::ga4gh_schemas::validate_drs_object(&v)?;
                validate_basic_drs_object(&fixture.object_id, &v)?;
                Ok(())
            })();
            TestCaseResult::from_outcome(
                "DRS DrsObject OpenAPI + access_methods",
                ComplianceLevel::Level1,
                TestCategory::Schema,
                res,
            )
        }
        Err(e) => TestCaseResult::from_outcome(
            "DRS DrsObject OpenAPI + access_methods",
            ComplianceLevel::Level1,
            TestCategory::Schema,
            Err(e),
        ),
    }
}

async fn level1_openapi_specsource(
    cfg: &TestConfig,
    client: &HttpClient,
    spec: &SpecSource,
    fixture: &DrsTestFixture,
) -> TestCaseResult {
    let url = object_url(cfg, &fixture.object_id);
    match get_object(client, &url).await {
        Ok(ObjectGet::NotFound { status }) => skip_fixture(
            DRS_OPENAPI_SPECSOURCE_CHECK,
            ComplianceLevel::Level1,
            TestCategory::Schema,
            format!(
                "GET {url} returned {status} (object_id={})",
                fixture.object_id
            ),
        ),
        Ok(ObjectGet::Json(v)) => {
            let res = common::ga4gh_schemas::validate_drs_object_with(spec, &v).map(|_| ());
            TestCaseResult::from_outcome(
                DRS_OPENAPI_SPECSOURCE_CHECK,
                ComplianceLevel::Level1,
                TestCategory::Schema,
                res,
            )
        }
        Err(e) => TestCaseResult::from_outcome(
            DRS_OPENAPI_SPECSOURCE_CHECK,
            ComplianceLevel::Level1,
            TestCategory::Schema,
            Err(e),
        ),
    }
}

async fn level1_basic_schema_and_fields_with_spec(
    cfg: &TestConfig,
    client: &HttpClient,
    spec: &SpecSource,
    fixture: &DrsTestFixture,
) -> TestCaseResult {
    let url = object_url(cfg, &fixture.object_id);
    match get_object(client, &url).await {
        Ok(ObjectGet::NotFound { status }) => skip_fixture(
            "DRS DrsObject OpenAPI + access_methods",
            ComplianceLevel::Level1,
            TestCategory::Schema,
            format!(
                "GET {url} returned {status} (object_id={})",
                fixture.object_id
            ),
        ),
        Ok(ObjectGet::Json(v)) => {
            let res: Result<()> = (|| {
                common::ga4gh_schemas::validate_drs_object_with(spec, &v)?;
                validate_basic_drs_object(&fixture.object_id, &v)?;
                Ok(())
            })();
            TestCaseResult::from_outcome(
                "DRS DrsObject OpenAPI + access_methods",
                ComplianceLevel::Level1,
                TestCategory::Schema,
                res,
            )
        }
        Err(e) => TestCaseResult::from_outcome(
            "DRS DrsObject OpenAPI + access_methods",
            ComplianceLevel::Level1,
            TestCategory::Schema,
            Err(e),
        ),
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
    fixture: &DrsTestFixture,
) -> TestCaseResult {
    if !features.strict_drs_checksums {
        return TestCaseResult::skip(
            "DRS checksum correctness",
            ComplianceLevel::Level2,
            TestCategory::Checksum,
            "strict_drs_checksums=false in features",
        );
    }

    let url = object_url(cfg, &fixture.object_id);
    match get_object(client, &url).await {
        Ok(ObjectGet::NotFound { status }) => skip_fixture(
            "DRS checksum correctness",
            ComplianceLevel::Level2,
            TestCategory::Checksum,
            format!(
                "GET {url} returned {status} (object_id={})",
                fixture.object_id
            ),
        ),
        Ok(ObjectGet::Json(v)) => {
            if first_access_url(&v).is_err() {
                return skip_fixture(
                    "DRS checksum correctness",
                    ComplianceLevel::Level2,
                    TestCategory::Checksum,
                    format!(
                        "object_id={} has no access_url; checksum needs independently fetchable bytes",
                        fixture.object_id
                    ),
                );
            }
            let result = checksum_against_bytes(client, &v, fixture).await;
            if let Err(e) = &result {
                if e.to_string().contains(FIXTURE_UNAVAILABLE) {
                    return skip_fixture(
                        "DRS checksum correctness",
                        ComplianceLevel::Level2,
                        TestCategory::Checksum,
                        e,
                    );
                }
            }
            TestCaseResult::from_outcome(
                "DRS checksum correctness",
                ComplianceLevel::Level2,
                TestCategory::Checksum,
                result,
            )
        }
        Err(e) => TestCaseResult::from_outcome(
            "DRS checksum correctness",
            ComplianceLevel::Level2,
            TestCategory::Checksum,
            Err(e),
        ),
    }
}

async fn checksum_against_bytes(
    client: &HttpClient,
    v: &Value,
    fixture: &DrsTestFixture,
) -> Result<()> {
    let access_url = first_access_url(v)?;
    let bytes = download_capped(client, access_url, CHECKSUM_BODY_LIMIT).await?;
    let actual = sha256_bytes(&bytes);

    if let Some(expected) = fixture.expected_sha256.as_deref() {
        info!(expected = %expected, actual = %actual, "DRS checksum comparison against fixture digest");
        if !actual.eq_ignore_ascii_case(expected) {
            anyhow::bail!(
                "DRS checksum mismatch for {}: fixture expected {}, got {}",
                fixture.object_id,
                expected,
                actual
            );
        }
        return Ok(());
    }

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

    info!(expected = %expected_checksum, actual = %actual, "DRS checksum comparison from advertised digest vs download");
    if !actual.eq_ignore_ascii_case(expected_checksum) {
        anyhow::bail!(
            "DRS checksum mismatch for {}: advertised {}, got {}",
            fixture.object_id,
            expected_checksum,
            actual
        );
    }
    Ok(())
}

async fn download_capped(client: &HttpClient, url: &str, limit: usize) -> Result<Vec<u8>> {
    let resp = client.inner().get(url).send().await?;
    if !resp.status().is_success() {
        anyhow::bail!(
            "Failed to download DRS object for checksum validation: {}",
            resp.status()
        );
    }
    if let Some(len) = resp.content_length() {
        if len > limit as u64 {
            anyhow::bail!(
                "{FIXTURE_UNAVAILABLE}: download Content-Length {len} exceeds {limit} bytes"
            );
        }
    }
    read_body_capped(resp, limit).await
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

async fn level2_range_request(
    cfg: &TestConfig,
    client: &HttpClient,
    fixture: &DrsTestFixture,
) -> TestCaseResult {
    let url = object_url(cfg, &fixture.object_id);
    match get_object(client, &url).await {
        Ok(ObjectGet::NotFound { status }) => skip_fixture(
            "DRS HTTP Range support",
            ComplianceLevel::Level2,
            TestCategory::Interoperability,
            format!(
                "GET {url} returned {status} (object_id={})",
                fixture.object_id
            ),
        ),
        Ok(ObjectGet::Json(v)) => {
            if first_access_url(&v).is_err() {
                return skip_fixture(
                    "DRS HTTP Range support",
                    ComplianceLevel::Level2,
                    TestCategory::Interoperability,
                    format!(
                        "object_id={} has no access_url; Range needs independently fetchable bytes",
                        fixture.object_id
                    ),
                );
            }
            let result = range_protocol(client, &v).await;
            TestCaseResult::from_outcome(
                "DRS HTTP Range support",
                ComplianceLevel::Level2,
                TestCategory::Interoperability,
                result,
            )
        }
        Err(e) => TestCaseResult::from_outcome(
            "DRS HTTP Range support",
            ComplianceLevel::Level2,
            TestCategory::Interoperability,
            Err(e),
        ),
    }
}

async fn range_protocol(client: &HttpClient, v: &Value) -> Result<()> {
    let access_url = first_access_url(v)?;
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
    Ok(())
}

async fn level5_invalid_id_404(
    cfg: &TestConfig,
    client: &HttpClient,
    fixture: &DrsTestFixture,
) -> TestCaseResult {
    let unknown = fixture.unknown_object_id();
    let url = object_url(cfg, &unknown);
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
