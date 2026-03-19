//! GA4GH htsget 1.3.0 checks (Ferrum gateway: `/ga4gh/htsget/v1`).
//!
//! Aligns with [Ferrum `ferrum-htsget`](https://github.com/SynapticFour/Ferrum): `reads/service-info`,
//! `variants/service-info`, GET/POST tickets, DRS stream URLs, and error codes.

use anyhow::Result;
use common::config::TestConfig;
use common::ga4gh_schemas::{
    validate_htsget_error, validate_htsget_service_info, validate_htsget_ticket_reads,
    validate_htsget_ticket_variants,
};
use common::http::HttpClient;
use common::report::{ComplianceLevel, ServiceKind, ServiceReport, TestCaseResult, TestCategory};
use serde_json::Value;
use tracing::info;
use url::Url;

use crate::{Features, Mode};

/// Default BAM/reads object (HelixTest DRS seeds; Ferrum demo).
const DEFAULT_READS_OBJECT_ID: &str = "test-object-1";
/// Default VCF object (Ferrum E2E / demo: `demo-sample-vcf`).
const DEFAULT_VARIANTS_OBJECT_ID: &str = "demo-sample-vcf";

const GA4GH_SERVICE_SUFFIXES: &[&str] = &[
    "/ga4gh/wes/v1",
    "/ga4gh/tes/v1",
    "/ga4gh/drs/v1",
    "/ga4gh/trs/v2",
    "/ga4gh/beacon/v2",
    "/ga4gh/htsget/v1",
    "/passports/v1",
];

/// Strip a known GA4GH service path → gateway origin (scheme + host + port).
fn gateway_from_ga4gh_service_url(service_url: &str) -> Option<String> {
    let u = service_url.trim_end_matches('/');
    for sfx in GA4GH_SERVICE_SUFFIXES {
        if let Some(base) = u.strip_suffix(sfx) {
            return Some(base.to_string());
        }
    }
    None
}

/// Bare `http://host:port` (path `/` or empty) → treat whole URL as gateway base.
fn gateway_from_bare_url(service_url: &str) -> Option<String> {
    let u = Url::parse(service_url.trim()).ok()?;
    let path = u.path();
    if path == "/" || path.is_empty() {
        let host = u.host_str()?;
        let scheme = u.scheme();
        let port = u.port().map(|p| format!(":{}", p)).unwrap_or_default();
        return Some(format!("{}://{}{}", scheme, host, port));
    }
    None
}

fn first_gateway_base(cfg: &TestConfig, mode: Mode) -> Option<String> {
    for url in [
        &cfg.services.wes_url,
        &cfg.services.drs_url,
        &cfg.services.tes_url,
        &cfg.services.trs_url,
    ] {
        if let Some(g) = gateway_from_ga4gh_service_url(url) {
            return Some(g);
        }
        // Bare `http://host:port` only implies a unified gateway (Ferrum); generic split mocks skip htsget.
        if matches!(mode, Mode::Ferrum) {
            if let Some(g) = gateway_from_bare_url(url) {
                return Some(g);
            }
        }
    }
    None
}

/// Resolve htsget API base including `/ga4gh/htsget/v1`.
pub fn resolve_htsget_base(cfg: &TestConfig, mode: Mode) -> Option<String> {
    if let Some(ref u) = cfg.services.htsget_url {
        let t = u.trim();
        if !t.is_empty() {
            return Some(t.trim_end_matches('/').to_string());
        }
    }
    if let Ok(u) = std::env::var("HTSGET_URL") {
        let t = u.trim();
        if !t.is_empty() {
            return Some(t.trim_end_matches('/').to_string());
        }
    }
    if let Ok(g) = std::env::var("GATEWAY_BASE") {
        let g = g.trim().trim_end_matches('/');
        if !g.is_empty() {
            return Some(format!("{}/ga4gh/htsget/v1", g));
        }
    }
    first_gateway_base(cfg, mode).map(|g| format!("{}/ga4gh/htsget/v1", g.trim_end_matches('/')))
}

fn skip(
    name: &str,
    level: ComplianceLevel,
    category: TestCategory,
    msg: impl Into<String>,
) -> TestCaseResult {
    let msg = msg.into();
    info!(test = name, reason = %msg, "htsget check skipped");
    TestCaseResult {
        name: name.to_string(),
        level,
        passed: true,
        error: Some(msg),
        category,
        weight: 1.0,
    }
}

fn fail(
    name: &str,
    level: ComplianceLevel,
    category: TestCategory,
    msg: impl Into<String>,
) -> TestCaseResult {
    TestCaseResult {
        name: name.to_string(),
        level,
        passed: false,
        error: Some(msg.into()),
        category,
        weight: 1.0,
    }
}

fn reads_object_id() -> String {
    std::env::var("HTSGET_READS_OBJECT_ID")
        .or_else(|_| std::env::var("HTSGET_READS_ID"))
        .unwrap_or_else(|_| DEFAULT_READS_OBJECT_ID.to_string())
}

fn variants_object_id() -> String {
    std::env::var("HTSGET_VARIANTS_OBJECT_ID")
        .unwrap_or_else(|_| DEFAULT_VARIANTS_OBJECT_ID.to_string())
}

fn urlencoding_encode_path_segment(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{:02X}", b)),
        }
    }
    out
}

fn htsget_error_code(v: &Value) -> Option<&str> {
    v.get("htsget")?.get("error")?.as_str()
}

/// Ticket success responses use GA4GH htsget JSON media type (Ferrum) or generic JSON for errors.
fn content_type_ok_for_ticket(ct: &str) -> bool {
    let l = ct.to_ascii_lowercase();
    l.contains("application/vnd.ga4gh.htsget") || l.contains("application/json")
}

fn path_is_drs_stream_ticket(path: &str) -> bool {
    // /ga4gh/drs/v1/objects/<encoded-id>/stream
    let p = path.trim_end_matches('/');
    p.ends_with("/stream") && p.contains("/ga4gh/drs/v1/objects/")
}

fn first_ticket_url_is_drs_stream(ticket: &Value) -> bool {
    let url_str = ticket
        .get("htsget")
        .and_then(|h| h.get("urls"))
        .and_then(|u| u.as_array())
        .and_then(|a| a.first())
        .and_then(|e| e.get("url"))
        .and_then(|u| u.as_str());
    let Some(url_str) = url_str else {
        return false;
    };
    let Ok(parsed) = Url::parse(url_str) else {
        return false;
    };
    path_is_drs_stream_ticket(parsed.path())
}

/// Official `htsgetServiceInfo` OpenAPI schema + endpoint-specific expectations.
fn validate_reads_service_info_response(v: &Value) -> Result<(), String> {
    validate_htsget_service_info(v).map_err(|e| e.to_string())?;
    let typ = v.get("type").ok_or_else(|| "missing type".to_string())?;
    if typ.get("version").and_then(|x| x.as_str()) != Some("1.3.0") {
        return Err(format!(
            "type.version must be 1.3.0 (htsget API spec), got {:?} — see GA4GH htsget 1.3.0",
            typ.get("version")
        ));
    }
    let h = v.get("htsget").ok_or_else(|| "missing htsget".to_string())?;
    if h.get("datatype").and_then(|x| x.as_str()) != Some("reads") {
        return Err(format!(
            "/reads/service-info: htsget.datatype must be reads, got {:?}",
            h.get("datatype")
        ));
    }
    let formats = h
        .get("formats")
        .and_then(|x| x.as_array())
        .ok_or_else(|| "htsget.formats missing".to_string())?;
    let fmt_strs: Vec<&str> = formats.iter().filter_map(|x| x.as_str()).collect();
    if !fmt_strs.iter().any(|s| *s == "BAM") {
        return Err(format!(
            "/reads/service-info: formats must include BAM, got {:?}",
            fmt_strs
        ));
    }
    Ok(())
}

fn validate_variants_service_info_response(v: &Value) -> Result<(), String> {
    validate_htsget_service_info(v).map_err(|e| e.to_string())?;
    let typ = v.get("type").ok_or_else(|| "missing type".to_string())?;
    if typ.get("version").and_then(|x| x.as_str()) != Some("1.3.0") {
        return Err(format!(
            "type.version must be 1.3.0 (htsget API spec), got {:?}",
            typ.get("version")
        ));
    }
    let h = v.get("htsget").ok_or_else(|| "missing htsget".to_string())?;
    if h.get("datatype").and_then(|x| x.as_str()) != Some("variants") {
        return Err(format!(
            "/variants/service-info: htsget.datatype must be variants, got {:?}",
            h.get("datatype")
        ));
    }
    let formats = h
        .get("formats")
        .and_then(|x| x.as_array())
        .ok_or_else(|| "htsget.formats missing".to_string())?;
    let fmt_strs: Vec<&str> = formats.iter().filter_map(|x| x.as_str()).collect();
    if !fmt_strs.iter().any(|s| *s == "VCF") && !fmt_strs.iter().any(|s| *s == "BCF") {
        return Err(format!(
            "/variants/service-info: formats must include VCF or BCF, got {:?}",
            fmt_strs
        ));
    }
    Ok(())
}

async fn fetch_json(
    client: &HttpClient,
    url: &str,
) -> Result<(reqwest::StatusCode, String, Value), String> {
    let resp = client
        .inner()
        .get(url)
        .send()
        .await
        .map_err(|e| format!("GET {} failed: {}", url, e))?;
    let status = resp.status();
    let text = resp.text().await.map_err(|e| format!("read body: {}", e))?;
    let v: Value = serde_json::from_str(&text).map_err(|e| {
        format!(
            "invalid JSON from {}: {} — body prefix: {:.200}",
            url,
            e,
            text.chars().take(200).collect::<String>()
        )
    })?;
    Ok((status, text, v))
}

async fn fetch_json_post(
    client: &HttpClient,
    url: &str,
    body: &Value,
) -> Result<(reqwest::StatusCode, String, Value), String> {
    let resp = client
        .inner()
        .post(url)
        .header("Content-Type", "application/json")
        .json(body)
        .send()
        .await
        .map_err(|e| format!("POST {} failed: {}", url, e))?;
    let status = resp.status();
    let text = resp.text().await.map_err(|e| format!("read body: {}", e))?;
    let v: Value = serde_json::from_str(&text).map_err(|e| {
        format!(
            "invalid JSON from {}: {} — body prefix: {:.200}",
            url,
            e,
            text.chars().take(200).collect::<String>()
        )
    })?;
    Ok((status, text, v))
}

async fn level0_reads_service_info(base: &str, client: &HttpClient) -> TestCaseResult {
    let url = format!("{}/reads/service-info", base.trim_end_matches('/'));
    let name = "htsget reads /reads/service-info (htsget 1.3.0)";
    match fetch_json(client, &url).await {
        Ok((status, _, v)) if status.is_success() => match validate_reads_service_info_response(&v) {
            Ok(()) => TestCaseResult {
                name: name.into(),
                level: ComplianceLevel::Level0,
                passed: true,
                error: None,
                category: TestCategory::Schema,
                weight: 1.0,
            },
            Err(e) => fail(name, ComplianceLevel::Level0, TestCategory::Schema, e),
        },
        Ok((status, text, _)) => fail(
            name,
            ComplianceLevel::Level0,
            TestCategory::Schema,
            format!(
                "GET {} returned HTTP {} — expected 200 and GA4GH htsget 1.3.0 reads service-info JSON. Body prefix: {:.200}",
                url,
                status,
                text.chars().take(200).collect::<String>()
            ),
        ),
        Err(e) => fail(name, ComplianceLevel::Level0, TestCategory::Schema, e),
    }
}

async fn level0_variants_service_info(base: &str, client: &HttpClient) -> TestCaseResult {
    let url = format!("{}/variants/service-info", base.trim_end_matches('/'));
    let name = "htsget variants /variants/service-info (htsget 1.3.0)";
    match fetch_json(client, &url).await {
        Ok((status, _, v)) if status.is_success() => match validate_variants_service_info_response(&v) {
            Ok(()) => TestCaseResult {
                name: name.into(),
                level: ComplianceLevel::Level0,
                passed: true,
                error: None,
                category: TestCategory::Schema,
                weight: 1.0,
            },
            Err(e) => fail(name, ComplianceLevel::Level0, TestCategory::Schema, e),
        },
        Ok((status, text, _)) => fail(
            name,
            ComplianceLevel::Level0,
            TestCategory::Schema,
            format!(
                "GET {} returned HTTP {} — expected 200 and variants service-info. Body prefix: {:.200}",
                url,
                status,
                text.chars().take(200).collect::<String>()
            ),
        ),
        Err(e) => fail(name, ComplianceLevel::Level0, TestCategory::Schema, e),
    }
}

async fn level1_get_reads_ticket(base: &str, client: &HttpClient) -> TestCaseResult {
    let id = reads_object_id();
    let url = format!(
        "{}/reads/{}",
        base.trim_end_matches('/'),
        urlencoding_encode_path_segment(&id)
    );
    let name = "htsget GET reads ticket (BAM + DRS stream URL)";
    let resp = match client.inner().get(&url).send().await {
        Ok(r) => r,
        Err(e) => {
            return fail(
                name,
                ComplianceLevel::Level1,
                TestCategory::Schema,
                format!("GET {} failed: {}", url, e),
            );
        }
    };
    let status = resp.status();
    let ct = resp
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|h| h.to_str().ok())
        .unwrap_or("")
        .to_string();
    let text = match resp.text().await {
        Ok(t) => t,
        Err(e) => {
            return fail(
                name,
                ComplianceLevel::Level1,
                TestCategory::Schema,
                format!("read body: {}", e),
            );
        }
    };
    if !status.is_success() {
        return fail(
            name,
            ComplianceLevel::Level1,
            TestCategory::Schema,
            format!(
                "GET {} returned HTTP {} — expected 200 for reads object {:?}. Body: {:.300}",
                url,
                status,
                id,
                text.chars().take(300).collect::<String>()
            ),
        );
    }
    if !content_type_ok_for_ticket(&ct) {
        return fail(
            name,
            ComplianceLevel::Level1,
            TestCategory::Schema,
            format!(
                "Content-Type should be application/vnd.ga4gh.htsget… or JSON, got {:?}",
                ct
            ),
        );
    }
    let v: Value = match serde_json::from_str(&text) {
        Ok(j) => j,
        Err(e) => {
            return fail(
                name,
                ComplianceLevel::Level1,
                TestCategory::Schema,
                format!("JSON parse error: {}", e),
            );
        }
    };
    if let Err(e) = validate_htsget_ticket_reads(&v) {
        return fail(
            name,
            ComplianceLevel::Level1,
            TestCategory::Schema,
            format!("ticket OpenAPI (htsgetResponseReads): {}", e),
        );
    }
    if !first_ticket_url_is_drs_stream(&v) {
        return fail(
            name,
            ComplianceLevel::Level1,
            TestCategory::Interoperability,
            format!(
                "first urls[].url must target …/ga4gh/drs/v1/objects/{{id}}/stream — got {:?}",
                v.get("htsget")
                    .and_then(|h| h.get("urls"))
                    .and_then(|u| u.get(0))
                    .and_then(|e| e.get("url"))
            ),
        );
    }
    TestCaseResult {
        name: name.into(),
        level: ComplianceLevel::Level1,
        passed: true,
        error: None,
        category: TestCategory::Schema,
        weight: 1.0,
    }
}

async fn level1_get_variants_ticket(base: &str, client: &HttpClient) -> TestCaseResult {
    let id = variants_object_id();
    let url = format!(
        "{}/variants/{}",
        base.trim_end_matches('/'),
        urlencoding_encode_path_segment(&id)
    );
    let name = "htsget GET variants ticket (VCF/BCF + DRS stream URL)";
    let resp = match client.inner().get(&url).send().await {
        Ok(r) => r,
        Err(e) => {
            return fail(
                name,
                ComplianceLevel::Level1,
                TestCategory::Schema,
                format!("GET {} failed: {}", url, e),
            );
        }
    };
    let status = resp.status();
    let ct = resp
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|h| h.to_str().ok())
        .unwrap_or("")
        .to_string();
    let text = match resp.text().await {
        Ok(t) => t,
        Err(e) => {
            return fail(
                name,
                ComplianceLevel::Level1,
                TestCategory::Schema,
                format!("read body: {}", e),
            );
        }
    };
    if !status.is_success() {
        return fail(
            name,
            ComplianceLevel::Level1,
            TestCategory::Schema,
            format!(
                "GET {} returned HTTP {} — expected 200 for VCF object {:?} (set HTSGET_VARIANTS_OBJECT_ID if your seed differs). Body: {:.300}",
                url, status, id, text.chars().take(300).collect::<String>()
            ),
        );
    }
    if !content_type_ok_for_ticket(&ct) {
        return fail(
            name,
            ComplianceLevel::Level1,
            TestCategory::Schema,
            format!(
                "Content-Type should be application/vnd.ga4gh.htsget… or JSON, got {:?}",
                ct
            ),
        );
    }
    let v: Value = match serde_json::from_str(&text) {
        Ok(j) => j,
        Err(e) => {
            return fail(
                name,
                ComplianceLevel::Level1,
                TestCategory::Schema,
                format!("JSON parse error: {}", e),
            );
        }
    };
    if let Err(e) = validate_htsget_ticket_variants(&v) {
        return fail(
            name,
            ComplianceLevel::Level1,
            TestCategory::Schema,
            format!("ticket OpenAPI (htsgetResponseVariants): {}", e),
        );
    }
    if !first_ticket_url_is_drs_stream(&v) {
        return fail(
            name,
            ComplianceLevel::Level1,
            TestCategory::Interoperability,
            format!(
                "first urls[].url must target …/ga4gh/drs/v1/objects/{{id}}/stream — got {:?}",
                v.get("htsget")
                    .and_then(|x| x.get("urls"))
                    .and_then(|x| x.get(0))
            ),
        );
    }
    TestCaseResult {
        name: name.into(),
        level: ComplianceLevel::Level1,
        passed: true,
        error: None,
        category: TestCategory::Schema,
        weight: 1.0,
    }
}

/// GET variants with a reads-only object → 404 NotFound.
async fn level2_variants_endpoint_wrong_kind(base: &str, client: &HttpClient) -> TestCaseResult {
    let id = reads_object_id();
    let url = format!(
        "{}/variants/{}",
        base.trim_end_matches('/'),
        urlencoding_encode_path_segment(&id)
    );
    let name = "htsget GET variants with reads-only object → NotFound";
    match fetch_json(client, &url).await {
        Ok((status, _, v)) if status.as_u16() == 404 => {
            if let Err(e) = validate_htsget_error(&v) {
                return fail(
                    name,
                    ComplianceLevel::Level2,
                    TestCategory::Robustness,
                    format!("404 error body OpenAPI: {}", e),
                );
            }
            if htsget_error_code(&v) == Some("NotFound") {
                TestCaseResult {
                    name: name.into(),
                    level: ComplianceLevel::Level2,
                    passed: true,
                    error: None,
                    category: TestCategory::Robustness,
                    weight: 1.0,
                }
            } else {
                fail(
                    name,
                    ComplianceLevel::Level2,
                    TestCategory::Robustness,
                    format!("expected htsget.error NotFound, got {:?}", v),
                )
            }
        }
        Ok((status, text, _v)) => fail(
            name,
            ComplianceLevel::Level2,
            TestCategory::Robustness,
            format!(
                "expected HTTP 404 + NotFound for variants/{} (BAM id), got {} body: {:.200}",
                id,
                status,
                text.chars().take(200).collect::<String>()
            ),
        ),
        Err(e) => fail(name, ComplianceLevel::Level2, TestCategory::Robustness, e),
    }
}

async fn level2_post_reads_ticket(base: &str, client: &HttpClient) -> TestCaseResult {
    let id = reads_object_id();
    let url = format!(
        "{}/reads/{}",
        base.trim_end_matches('/'),
        urlencoding_encode_path_segment(&id)
    );
    let name = "htsget POST reads ticket (JSON body, no query)";
    let body = serde_json::json!({
        "format": "BAM",
        "regions": [{"referenceName": "chr1", "start": 0, "end": 1000}]
    });
    let resp = match client
        .inner()
        .post(&url)
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => {
            return fail(
                name,
                ComplianceLevel::Level2,
                TestCategory::Interoperability,
                format!("POST failed: {}", e),
            )
        }
    };
    let status = resp.status();
    let text = match resp.text().await {
        Ok(t) => t,
        Err(e) => {
            return fail(
                name,
                ComplianceLevel::Level2,
                TestCategory::Interoperability,
                format!("read body: {}", e),
            )
        }
    };
    if !status.is_success() {
        return fail(
            name,
            ComplianceLevel::Level2,
            TestCategory::Interoperability,
            format!(
                "POST {} expected 2xx, got {} body {:.300}",
                url,
                status,
                text.chars().take(300).collect::<String>()
            ),
        );
    }
    let v: Value = match serde_json::from_str(&text) {
        Ok(j) => j,
        Err(e) => {
            return fail(
                name,
                ComplianceLevel::Level2,
                TestCategory::Interoperability,
                format!("JSON: {}", e),
            )
        }
    };
    if let Err(e) = validate_htsget_ticket_reads(&v) {
        return fail(
            name,
            ComplianceLevel::Level2,
            TestCategory::Interoperability,
            format!("POST ticket OpenAPI (htsgetResponseReads): {}", e),
        );
    }
    if !first_ticket_url_is_drs_stream(&v) {
        return fail(
            name,
            ComplianceLevel::Level2,
            TestCategory::Interoperability,
            "POST ticket must include DRS stream URL like GET".to_string(),
        );
    }
    TestCaseResult {
        name: name.into(),
        level: ComplianceLevel::Level2,
        passed: true,
        error: None,
        category: TestCategory::Interoperability,
        weight: 1.0,
    }
}

async fn level2_post_variants_ticket(base: &str, client: &HttpClient) -> TestCaseResult {
    let id = variants_object_id();
    let url = format!(
        "{}/variants/{}",
        base.trim_end_matches('/'),
        urlencoding_encode_path_segment(&id)
    );
    let name = "htsget POST variants ticket (JSON body, no query)";
    let body = serde_json::json!({
        "format": "VCF",
        "regions": [{"referenceName": "chr1", "start": 0, "end": 500}]
    });
    let resp = match client
        .inner()
        .post(&url)
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => {
            return fail(
                name,
                ComplianceLevel::Level2,
                TestCategory::Interoperability,
                format!("POST failed: {}", e),
            );
        }
    };
    let status = resp.status();
    let text = match resp.text().await {
        Ok(t) => t,
        Err(e) => {
            return fail(
                name,
                ComplianceLevel::Level2,
                TestCategory::Interoperability,
                format!("read body: {}", e),
            );
        }
    };
    if !status.is_success() {
        return fail(
            name,
            ComplianceLevel::Level2,
            TestCategory::Interoperability,
            format!(
                "POST {} expected 2xx, got {} body {:.300}",
                url,
                status,
                text.chars().take(300).collect::<String>()
            ),
        );
    }
    let v: Value = match serde_json::from_str(&text) {
        Ok(j) => j,
        Err(e) => {
            return fail(
                name,
                ComplianceLevel::Level2,
                TestCategory::Interoperability,
                format!("JSON: {}", e),
            );
        }
    };
    if let Err(e) = validate_htsget_ticket_variants(&v) {
        return fail(
            name,
            ComplianceLevel::Level2,
            TestCategory::Interoperability,
            format!("POST ticket OpenAPI (htsgetResponseVariants): {}", e),
        );
    }
    if !first_ticket_url_is_drs_stream(&v) {
        return fail(
            name,
            ComplianceLevel::Level2,
            TestCategory::Interoperability,
            "POST variants ticket must include DRS stream URL like GET".to_string(),
        );
    }
    TestCaseResult {
        name: name.into(),
        level: ComplianceLevel::Level2,
        passed: true,
        error: None,
        category: TestCategory::Interoperability,
        weight: 1.0,
    }
}

async fn level2_post_reads_with_query_invalid(base: &str, client: &HttpClient) -> TestCaseResult {
    let id = reads_object_id();
    let url = format!(
        "{}/reads/{}?format=BAM",
        base.trim_end_matches('/'),
        urlencoding_encode_path_segment(&id)
    );
    let name = "htsget POST reads with query params → InvalidInput";
    let body = serde_json::json!({"format": "BAM"});
    match fetch_json_post(client, &url, &body).await {
        Ok((status, _, v)) if status.as_u16() == 400 && htsget_error_code(&v) == Some("InvalidInput") => {
            if let Err(e) = validate_htsget_error(&v) {
                return fail(
                    name,
                    ComplianceLevel::Level2,
                    TestCategory::Robustness,
                    format!("error body OpenAPI: {}", e),
                );
            }
            TestCaseResult {
                name: name.into(),
                level: ComplianceLevel::Level2,
                passed: true,
                error: None,
                category: TestCategory::Robustness,
                weight: 1.0,
            }
        }
        Ok((status, text, v)) => fail(
            name,
            ComplianceLevel::Level2,
            TestCategory::Robustness,
            format!(
                "expected HTTP 400 and htsget.error InvalidInput for POST with query string, got {} {:?} {:.200}",
                status,
                htsget_error_code(&v),
                text.chars().take(200).collect::<String>()
            ),
        ),
        Err(e) => fail(name, ComplianceLevel::Level2, TestCategory::Robustness, e),
    }
}

async fn level2_get_unsupported_format_cram_on_bam(
    base: &str,
    client: &HttpClient,
) -> TestCaseResult {
    let id = reads_object_id();
    let name = "htsget GET reads ?format=CRAM on BAM object → UnsupportedFormat";
    let plain_url = format!(
        "{}/reads/{}",
        base.trim_end_matches('/'),
        urlencoding_encode_path_segment(&id)
    );
    if let Ok((st, _, v)) = fetch_json(client, &plain_url).await {
        if st.is_success() {
            let fmt = v
                .get("htsget")
                .and_then(|h| h.get("format"))
                .and_then(|x| x.as_str());
            if fmt == Some("CRAM") {
                return skip(
                    name,
                    ComplianceLevel::Level2,
                    TestCategory::Robustness,
                    "skipped: reads object reports format CRAM — use a BAM-backed HTSGET_READS_OBJECT_ID to assert UnsupportedFormat for ?format=CRAM",
                );
            }
        }
    }
    let url = format!("{}?format=CRAM", plain_url);
    match fetch_json(client, &url).await {
        Ok((status, _, v)) if status.as_u16() == 400 && htsget_error_code(&v) == Some("UnsupportedFormat") => {
            if let Err(e) = validate_htsget_error(&v) {
                return fail(
                    name,
                    ComplianceLevel::Level2,
                    TestCategory::Robustness,
                    format!("error body OpenAPI: {}", e),
                );
            }
            TestCaseResult {
                name: name.into(),
                level: ComplianceLevel::Level2,
                passed: true,
                error: None,
                category: TestCategory::Robustness,
                weight: 1.0,
            }
        }
        Ok((status, text, v)) => fail(
            name,
            ComplianceLevel::Level2,
            TestCategory::Robustness,
            format!(
                "expected HTTP 400 UnsupportedFormat for CRAM on BAM object {:?}, got {} err={:?} body {:.200}",
                id,
                status,
                htsget_error_code(&v),
                text.chars().take(200).collect::<String>()
            ),
        ),
        Err(e) => fail(name, ComplianceLevel::Level2, TestCategory::Robustness, e),
    }
}

async fn level2_get_class_header_invalid(base: &str, client: &HttpClient) -> TestCaseResult {
    let id = reads_object_id();
    let url = format!(
        "{}/reads/{}?class=header",
        base.trim_end_matches('/'),
        urlencoding_encode_path_segment(&id)
    );
    let name = "htsget GET reads ?class=header → InvalidInput";
    match fetch_json(client, &url).await {
        Ok((status, _, v))
            if status.as_u16() == 400 && htsget_error_code(&v) == Some("InvalidInput") =>
        {
            if let Err(e) = validate_htsget_error(&v) {
                return fail(
                    name,
                    ComplianceLevel::Level2,
                    TestCategory::Robustness,
                    format!("error body OpenAPI: {}", e),
                );
            }
            TestCaseResult {
                name: name.into(),
                level: ComplianceLevel::Level2,
                passed: true,
                error: None,
                category: TestCategory::Robustness,
                weight: 1.0,
            }
        }
        Ok((status, text, v)) => fail(
            name,
            ComplianceLevel::Level2,
            TestCategory::Robustness,
            format!(
                "expected HTTP 400 InvalidInput for class=header, got {} err={:?} {:.200}",
                status,
                htsget_error_code(&v),
                text.chars().take(200).collect::<String>()
            ),
        ),
        Err(e) => fail(name, ComplianceLevel::Level2, TestCategory::Robustness, e),
    }
}

/// When `HELIXTEST_HTSGET_DATASET_OBJECT_ID` is set: no Bearer → 403 PermissionDenied; with `HELIXTEST_HTSGET_DATASET_BEARER` → 200 ticket.
async fn level4_htsget_dataset_auth(base: &str, client: &HttpClient) -> TestCaseResult {
    let name = "htsget dataset auth (403 without token, 200 with Passport/JWT)";
    let obj = match std::env::var("HELIXTEST_HTSGET_DATASET_OBJECT_ID") {
        Ok(s) if !s.trim().is_empty() => s,
        _ => {
            return skip(
                name,
                ComplianceLevel::Level4,
                TestCategory::Security,
                "skipped: set HELIXTEST_HTSGET_DATASET_OBJECT_ID (and HELIXTEST_HTSGET_DATASET_BEARER for success path) to test Ferrum dataset-gated htsget when FERRUM_AUTH__REQUIRE_AUTH=true",
            );
        }
    };
    let url = format!(
        "{}/reads/{}",
        base.trim_end_matches('/'),
        urlencoding_encode_path_segment(&obj.trim())
    );
    let no_auth = match fetch_json(client, &url).await {
        Ok(x) => x,
        Err(e) => {
            return fail(name, ComplianceLevel::Level4, TestCategory::Security, e);
        }
    };
    let (status_no, _, v_no) = no_auth;
    if status_no.as_u16() != 403 || htsget_error_code(&v_no) != Some("PermissionDenied") {
        return fail(
            name,
            ComplianceLevel::Level4,
            TestCategory::Security,
            format!(
                "without Bearer: expected HTTP 403 and htsget.error PermissionDenied for dataset object {:?}, got {} {:?}",
                obj,
                status_no,
                htsget_error_code(&v_no)
            ),
        );
    }
    if let Err(e) = validate_htsget_error(&v_no) {
        return fail(
            name,
            ComplianceLevel::Level4,
            TestCategory::Security,
            format!("403 error body OpenAPI: {}", e),
        );
    }
    let token = match std::env::var("HELIXTEST_HTSGET_DATASET_BEARER") {
        Ok(s) if !s.trim().is_empty() => s,
        _ => {
            return TestCaseResult {
                name: name.into(),
                level: ComplianceLevel::Level4,
                passed: true,
                error: Some(
                    "partial: 403 without token OK — set HELIXTEST_HTSGET_DATASET_BEARER (GA4GH Passport or token with ControlledAccessGrants visa) to assert 200 ticket"
                        .into(),
                ),
                category: TestCategory::Security,
                weight: 1.0,
            };
        }
    };
    let resp = match client
        .inner()
        .get(&url)
        .header("Authorization", format!("Bearer {}", token.trim()))
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => {
            return fail(
                name,
                ComplianceLevel::Level4,
                TestCategory::Security,
                format!("GET with Bearer failed: {}", e),
            );
        }
    };
    let status = resp.status();
    let text = match resp.text().await {
        Ok(t) => t,
        Err(e) => {
            return fail(
                name,
                ComplianceLevel::Level4,
                TestCategory::Security,
                format!("read body: {}", e),
            );
        }
    };
    if !status.is_success() {
        return fail(
            name,
            ComplianceLevel::Level4,
            TestCategory::Security,
            format!(
                "with Bearer: expected 200 ticket, got {} body {:.300}",
                status,
                text.chars().take(300).collect::<String>()
            ),
        );
    }
    let v: Value = match serde_json::from_str(&text) {
        Ok(j) => j,
        Err(e) => {
            return fail(
                name,
                ComplianceLevel::Level4,
                TestCategory::Security,
                format!("JSON: {}", e),
            );
        }
    };
    if let Err(e) = validate_htsget_ticket_reads(&v) {
        return fail(
            name,
            ComplianceLevel::Level4,
            TestCategory::Security,
            format!("ticket OpenAPI (htsgetResponseReads): {}", e),
        );
    }
    if !first_ticket_url_is_drs_stream(&v) {
        return fail(
            name,
            ComplianceLevel::Level4,
            TestCategory::Security,
            "authenticated ticket must include DRS stream URL".to_string(),
        );
    }
    TestCaseResult {
        name: name.into(),
        level: ComplianceLevel::Level4,
        passed: true,
        error: None,
        category: TestCategory::Security,
        weight: 1.0,
    }
}

pub async fn run_htsget_checks(
    mode: Mode,
    _features: &Features,
    cfg: &TestConfig,
    client: &HttpClient,
) -> Result<ServiceReport> {
    let mut tests = Vec::new();

    let Some(base) = resolve_htsget_base(cfg, mode) else {
        let hint = "Could not resolve htsget base: set GATEWAY_BASE, HTSGET_URL, [services] htsget, or use gateway-style WES_URL/DRS_URL (e.g. http://host:8080/ga4gh/wes/v1) as in profiles/ferrum.toml — see helixtest/docs/ferrum.md.";
        tests.push(skip(
            "htsget suite (service-info, tickets, POST, errors)",
            ComplianceLevel::Level0,
            TestCategory::Other,
            hint,
        ));
        return Ok(ServiceReport {
            service: ServiceKind::Htsget,
            tests,
        });
    };

    tests.push(level0_reads_service_info(&base, client).await);
    tests.push(level0_variants_service_info(&base, client).await);
    tests.push(level1_get_reads_ticket(&base, client).await);
    tests.push(level1_get_variants_ticket(&base, client).await);
    tests.push(level2_variants_endpoint_wrong_kind(&base, client).await);
    tests.push(level2_post_reads_ticket(&base, client).await);
    tests.push(level2_post_variants_ticket(&base, client).await);
    tests.push(level2_post_reads_with_query_invalid(&base, client).await);
    tests.push(level2_get_unsupported_format_cram_on_bam(&base, client).await);
    tests.push(level2_get_class_header_invalid(&base, client).await);
    tests.push(level4_htsget_dataset_auth(&base, client).await);

    Ok(ServiceReport {
        service: ServiceKind::Htsget,
        tests,
    })
}
