// SPDX-License-Identifier: Apache-2.0
//! F1 lock: `run_drs_checks_with_spec` must validate against the supplied
//! SpecSource, not HelixTest's bundled OpenAPI.
//!
//! Wrong implementation attacked: `compile_identity(spec); validate_drs_object(...)`.
//! That would previously stay green (hashes from SpecSource, schema from bundled).
//! This test fails because bundled accepts the mock payload and the mutated
//! SpecSource rejects it; the schema check must reject.

use std::collections::BTreeMap;
use std::sync::Arc;

use common::config::{AuthChecksConfig, ServiceConfig, SubsetConfig, TestConfig};
use common::ga4gh_schemas::validate_drs_object;
use common::http::HttpClient;
use common::report::TestStatus;
use common::spec_source::{bundled_drs_validate_calls, reset_schema_call_counters, SpecSource};
use framework::drs::run_drs_checks_with_spec;
use framework::{Features, Mode};

#[allow(dead_code)]
#[path = "../../../testing/mock_ga4gh_drs.rs"]
mod mock_ga4gh_drs;

use mock_ga4gh_drs::start_mock_ga4gh_drs;

fn drs_cfg(drs_url: &str) -> TestConfig {
    TestConfig {
        services: ServiceConfig {
            wes_url: String::new(),
            tes_url: String::new(),
            drs_url: drs_url.to_string(),
            trs_url: String::new(),
            beacon_url: String::new(),
            auth_url: String::new(),
            htsget_url: None,
        },
        subset: SubsetConfig::default(),
        auth_checks: AuthChecksConfig::default(),
    }
}

fn file_map(pairs: &[(&str, &str)]) -> BTreeMap<String, Arc<[u8]>> {
    pairs
        .iter()
        .map(|(k, v)| (k.to_string(), Arc::<[u8]>::from(v.as_bytes())))
        .collect()
}

/// Minimal DrsObject closure plus an extra required property the mock JSON lacks.
fn mutated_spec_rejects_mock_payload() -> SpecSource {
    let files = file_map(&[
        (
            "openapi/components/schemas/DrsObject.yaml",
            "type: object\nrequired:\n  - id\n  - self_uri\n  - size\n  - created_time\n  - checksums\n  - deliberately_injected_field\nproperties:\n  id:\n    type: string\n  self_uri:\n    type: string\n  size:\n    type: integer\n  created_time:\n    type: string\n  checksums:\n    type: array\n    items:\n      $ref: './Checksum.yaml'\n  access_methods:\n    type: array\n    items:\n      $ref: './AccessMethod.yaml'\n  deliberately_injected_field:\n    type: string\n",
        ),
        (
            "openapi/components/schemas/Checksum.yaml",
            "type: object\nrequired: [checksum, type]\nproperties:\n  checksum:\n    type: string\n  type:\n    type: string\n",
        ),
        (
            "openapi/components/schemas/AccessMethod.yaml",
            "type: object\nrequired: [type]\nproperties:\n  type:\n    type: string\n  access_url:\n    $ref: './AccessURL.yaml'\n  authorizations:\n    $ref: './Authorizations.yaml'\n",
        ),
        (
            "openapi/components/schemas/AccessURL.yaml",
            "type: object\nproperties:\n  url:\n    type: string\n",
        ),
        (
            "openapi/components/schemas/Authorizations.yaml",
            "type: object\nproperties:\n  supported_types:\n    type: array\n    items:\n      type: string\n",
        ),
        (
            "openapi/components/schemas/ContentsObject.yaml",
            "type: object\nrequired: [name]\nproperties:\n  name:\n    type: string\n",
        ),
    ]);
    SpecSource {
        schema_entry: "openapi/components/schemas/DrsObject.yaml".into(),
        schema_component: "DrsObject".into(),
        files,
    }
}

#[tokio::test]
async fn test_with_spec_cannot_fallback_to_bundled_schema() {
    let mock = start_mock_ga4gh_drs().await;
    let cfg = drs_cfg(&mock.drs_url());
    let client = HttpClient::new();
    let features = Features {
        strict_drs_checksums: true,
        ..Features::default()
    };

    let url = format!("{}/objects/test-object-1", mock.drs_url());
    let payload = client.get_json(&url).await.expect("mock DrsObject JSON");
    validate_drs_object(&payload)
        .expect("bundled HelixTest OpenAPI must accept this mock payload (discriminator)");

    let spec = mutated_spec_rejects_mock_payload();
    reset_schema_call_counters();
    let bundled_before = bundled_drs_validate_calls();
    let (report, _compile) =
        run_drs_checks_with_spec(Mode::Generic, &features, &cfg, &client, &spec)
            .await
            .expect("with_spec must run");
    assert_eq!(
        bundled_drs_validate_calls(),
        bundled_before,
        "run_drs_checks_with_spec must not call validate_drs_object (bundled OpenAPI)"
    );

    let schema = report
        .tests
        .iter()
        .find(|t| t.name == "DRS DrsObject OpenAPI + access_methods")
        .expect("schema check");
    assert_eq!(
        schema.status,
        TestStatus::Fail,
        "mutated SpecSource must reject a payload the bundled schema accepts; \
         if this is Pass, with_spec fell back to validate_drs_object: {:?}",
        schema.error
    );
    let err = schema.error.as_deref().unwrap_or("");
    assert!(
        err.contains("deliberately_injected_field") || err.contains("required"),
        "schema fail must be the injected required field, got {err}"
    );
}
