// SPDX-License-Identifier: Apache-2.0
//! In-process GA4GH DRS 1.x HTTP fixture. Not Ferrum. Used by HelixTest to prove
//! generic DRS checks talk only to public DRS HTTP (GET `/objects/{id}`, bytes URL,
//! Range 206, unknown id 404).

use common::util::sha256_bytes;
use serde_json::json;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, Request, ResponseTemplate};

pub const TEST_OBJECT_ID: &str = "test-object-1";
pub const UNKNOWN_OBJECT_ID: &str = "nonexistent-object-id-for-conformance";
pub const BLOB_LEN: usize = 4096;

/// DRS checks executed by `run_drs_checks` (same set against Ferrum DRS and this mock).
pub const DRS_CHECK_NAMES: [&str; 5] = [
    "DRS object endpoint reachable",
    "DRS DrsObject OpenAPI + access_methods",
    "DRS checksum correctness",
    "DRS HTTP Range support",
    "DRS invalid object id returns 404",
];

pub struct MockGa4ghDrs {
    pub server: MockServer,
}

impl MockGa4ghDrs {
    pub fn drs_url(&self) -> String {
        self.server.uri()
    }
}

struct BytesWithOptionalRange {
    body: Vec<u8>,
}

impl wiremock::Respond for BytesWithOptionalRange {
    fn respond(&self, request: &Request) -> ResponseTemplate {
        let total = self.body.len() as u64;
        let range_hdr = request
            .headers
            .get("range")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");
        if let Some(spec) = range_hdr.strip_prefix("bytes=") {
            let (start_s, end_s) = spec.split_once('-').unwrap_or(("0", ""));
            let start: usize = start_s.parse().unwrap_or(0);
            let end: usize = if end_s.is_empty() {
                self.body.len().saturating_sub(1)
            } else {
                end_s.parse().unwrap_or(self.body.len().saturating_sub(1))
            };
            let end = end.min(self.body.len().saturating_sub(1));
            let start = start.min(end);
            let slice = self.body[start..=end].to_vec();
            return ResponseTemplate::new(206)
                .insert_header("Content-Range", format!("bytes {start}-{end}/{total}"))
                .insert_header("Content-Type", "application/octet-stream")
                .set_body_bytes(slice);
        }
        ResponseTemplate::new(200)
            .insert_header("Content-Type", "application/octet-stream")
            .set_body_bytes(self.body.clone())
    }
}

pub async fn start_mock_ga4gh_drs() -> MockGa4ghDrs {
    let blob = vec![b'A'; BLOB_LEN];
    let sha256 = sha256_bytes(&blob);
    let server = MockServer::start().await;
    let access_url = format!("{}/bytes/{TEST_OBJECT_ID}", server.uri());
    let object = json!({
        "id": TEST_OBJECT_ID,
        "name": TEST_OBJECT_ID,
        "self_uri": format!("drs://example.invalid/{TEST_OBJECT_ID}"),
        "size": BLOB_LEN,
        "created_time": "2020-01-01T00:00:00Z",
        "checksums": [{ "type": "sha256", "checksum": sha256 }],
        "access_methods": [{
            "type": "https",
            "access_url": { "url": access_url }
        }]
    });

    Mock::given(method("GET"))
        .and(path(format!("/objects/{TEST_OBJECT_ID}")))
        .respond_with(ResponseTemplate::new(200).set_body_json(object))
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path(format!("/objects/{UNKNOWN_OBJECT_ID}")))
        .respond_with(ResponseTemplate::new(404).set_body_string("not found"))
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path(format!("/bytes/{TEST_OBJECT_ID}")))
        .respond_with(BytesWithOptionalRange { body: blob })
        .mount(&server)
        .await;

    // WES-shaped service-info whose name contains "Ferrum". Generic mode must ignore this.
    Mock::given(method("GET"))
        .and(path("/service-info"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": "org.example.mock-wes",
            "name": "Ferrum Gateway",
            "type": { "group": "org.ga4gh", "artifact": "wes", "version": "1.1.0" }
        })))
        .mount(&server)
        .await;

    MockGa4ghDrs { server }
}
