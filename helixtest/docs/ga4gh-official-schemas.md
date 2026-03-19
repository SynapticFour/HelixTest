# Using GA4GH Official OpenAPI / JSON Schemas

This document outlines the changes needed to validate API responses against **official GA4GH OpenAPI/JSON Schema definitions** instead of HelixTest’s current Rust-derived schemas (from `schemars::schema_for!(T)`).

## Implemented: WES

- **Vendored:** `helixtest/schemas/ga4gh/wes-openapi.yaml` (WES 1.1.0 from [ga4gh.github.io](https://ga4gh.github.io/workflow-execution-service-schemas/openapi.yaml)).
- **Loading:** `common::ga4gh_schemas` parses the YAML, resolves `$ref` with `openapi-deref`, and compiles the **ServiceInfo** schema for validation.
- **Framework:** WES Level 1 (service-info) uses `validate_wes_service_info()` against this official schema; the “supported_wes_versions must contain 1.0 or 1.1” check is still applied.
- **Note:** The official ServiceInfo schema is strict (many required fields). Implementations that omit optional fields may need to extend their service-info to pass.

## Implemented: TES

- **Vendored:** `helixtest/schemas/ga4gh/tes-openapi.yaml` (TES 1.1.0 from [ga4gh/task-execution-schemas](https://github.com/ga4gh/task-execution-schemas)).
- **Loading:** Same pipeline as WES; schemas **tesTask** and **tesCreateTaskResponse** are extracted after `$ref` resolution (partial resolution is accepted when some refs are external).
- **Framework:** TES Level 1 uses `validate_tes_create_task_response()` for POST /tasks and `validate_tes_task()` for GET /tasks/{id}.

## Implemented: TRS

- **Vendored:** `helixtest/schemas/ga4gh/trs-openapi.yaml` (TRS 2.1.0 from [ga4gh/tool-registry-service-schemas](https://github.com/ga4gh/tool-registry-service-schemas), develop branch).
- **Loading:** **Tool** and **ToolVersion** schemas are extracted from the resolved OpenAPI (external refs may remain unresolved; in-document schemas are used).
- **Framework:** TRS Level 1 uses `validate_trs_tool()` for /tools (array items) and `validate_trs_tool_version()` for /tools/{id}/versions (array items).

## Implemented: htsget

- **Vendored:** `helixtest/schemas/ga4gh/htsget-openapi.yaml` (htsget **1.3.0** from [samtools/hts-specs](https://github.com/samtools/hts-specs) `pub/htsget-openapi.yaml`). The upstream `ServiceInfo` `$ref` to GA4GH Discovery is inlined as `Ga4ghService` / `Ga4ghServiceType` so `openapi_deref` works fully offline; `organization.url` allows `null` (Ferrum) via `oneOf` string URI \| null.
- **Loading:** Same pipeline as WES/TES; compiled schemas: **htsgetServiceInfo**, **htsgetResponseReads**, **htsgetResponseVariants**, **Error**.
- **Framework:** `framework/src/htsget.rs` calls `validate_htsget_service_info()` for reads/variants service-info (plus endpoint checks: `type.version == 1.3.0`, datatype, BAM / VCF|BCF), `validate_htsget_ticket_reads` / `validate_htsget_ticket_variants` for successful tickets (GET/POST and dataset-auth path), and `validate_htsget_error()` for JSON error bodies where applicable; DRS stream URL on the first ticket URL remains an extra interoperability rule.

## Current approach (DRS, Beacon)

- **DRS:** Level 1 uses manual field checks in `validate_basic_drs_object()` (id, self_uri, name, access_methods). The official DRS spec is multi-file; full schema validation could be added later by bundling or resolving those refs.
- **Beacon:** Level 1 uses Rust struct deserialization (`BeaconResponse` with meta/response). The Beacon v2 framework uses split JSON schemas with `$ref` to common/sections; official schema validation could be added by vendoring and resolving those assets.

## Goal

Validate responses using the **official GA4GH OpenAPI/JSON Schema** artifacts so that:

- Conformance is defined by the spec, not by our structs.
- New spec versions can be adopted by updating schema assets.
- Coverage matches the official definitions (with optional strictness profiles).

## GA4GH schema sources (reference)

- **WES:** [workflow-execution-service-schemas](https://ga4gh.github.io/workflow-execution-service-schemas/) — OpenAPI: `openapi.yaml` (or equivalent).
- **TES:** Task Execution Service schemas (search for `ga4gh/task-execution-service-schemas` or equivalent).
- **DRS:** Data Repository Service schemas (e.g. `ga4gh/data-repository-service-schemas`).
- **TRS:** Tool Registry Service schemas.
- **htsget:** [hts-specs](https://github.com/samtools/hts-specs) — `pub/htsget-openapi.yaml`.
- **Beacon:** Beacon v2 API schemas.

These are typically published as OpenAPI 3.x (YAML/JSON); components may be JSON Schema Draft 4/7 compatible or need a small conversion step.

## Changes required

### 1. Schema assets

- **Obtain official schemas:** For each service (WES, TES, DRS, TRS, Beacon), get the canonical OpenAPI or JSON Schema file(s) (e.g. from GA4GH GitHub repos or their published URLs).
- **Vendor or fetch:** Either:
  - **Vendor:** Add schema files under e.g. `helixtest/schemas/ga4gh/` (e.g. `wes-openapi.yaml`, `tes-openapi.yaml`) and load them at runtime or build time, or
  - **Fetch at build/test:** Script or build step that downloads the latest (or pinned) schema URLs into a generated directory and commit or use in CI.
- **OpenAPI → JSON Schema (if needed):** OpenAPI 3.x “schemas” are close to JSON Schema but not identical. Options:
  - Use a crate that validates against OpenAPI 3 directly (e.g. `openapi-spec` or similar), or
  - Convert OpenAPI components to standalone JSON Schema (e.g. via `openapi2jsonschema` or a small conversion script) and keep using the existing `jsonschema` crate.

### 2. Common schema loading

- **New module (e.g. `common::ga4gh_schemas`):**
  - Load schema content from files (or embedded) and parse into `serde_json::Value`.
  - Optionally support multiple spec versions (e.g. WES 1.0 vs 1.1) via path or config.
  - Expose a compiled `jsonschema::JSONSchema` (or equivalent) per endpoint/schema (e.g. “WES service-info”, “WES run status”, “TES task”).
- **Caching:** Compile schemas once per run (or lazily) to avoid re-parsing on every request.

### 3. Replace Rust-derived validation

- **WES:** In `framework/src/wes.rs`, instead of `schemars::schema_for!(WesServiceInfo)` and manual version checks:
  - Load the official WES OpenAPI (or the extracted “ServiceInfo” component schema).
  - Validate `/service-info` response against that schema.
  - Keep the “supported_wes_versions must contain 1.0 or 1.1” (or equivalent) as a **separate** conformance rule if it’s in the spec; otherwise derive from the schema (e.g. enum or pattern).
- **TES:** Replace `validate_json::<TesTaskCreateResponse>` (and similar) with validation against the official TES task/status schema(s).
- **TRS:** Same idea for tools/versions/descriptor schemas.
- **DRS:** Replace or supplement manual field checks with the official DRS object schema.
- **Beacon:** Use the official Beacon response schema(s) for `/query` (and related) responses.

### 4. Dependencies

- **Optional new crates:** If you validate against OpenAPI 3 directly: add a crate that can load OpenAPI and validate a JSON body against a path/operation (e.g. `openapi-validation` or similar; check crates.io).
- **Otherwise:** Keep `jsonschema`; add a small OpenAPI→JSON Schema conversion step (script or crate) and keep validation as today with “official” JSON Schema instances.

### 5. Versioning and profiles

- **Schema version selection:** Allow choosing spec version (e.g. WES 1.0 vs 1.1) via config or profile so implementations can target a specific version.
- **Strict vs lenient:** Optionally support “strict” (all required fields and formats from the spec) vs “lenient” (only critical fields) to accommodate partial implementations; document in [scoring](scoring.md) or [contributing](contributing.md).

### 6. Tests and CI

- **Unit tests:** Add tests that validate known-good JSON (from spec examples or fixtures) against the loaded official schema to catch regressions when updating schema files.
- **CI:** If schemas are fetched at build time, cache them or pin URLs/commits so CI is deterministic.

### 7. Documentation

- Update [contributing](contributing.md) and [README](../README.md) to mention that schema validation uses GA4GH official schemas and where they live (paths or URLs).
- Document how to update schemas when a new GA4GH spec version is released.

## Suggested order of work

1. **WES only (pilot):** Add `helixtest/schemas/ga4gh/` (or similar), add the official WES OpenAPI/schema, implement loading in `common`, and switch WES service-info validation to it. Remove or keep `WesServiceInfo` only for deserialization if needed.
2. **Generalize loading:** Factor a small “load and compile schema by service/endpoint” API in `common` and reuse for TES, DRS, TRS, Beacon.
3. **Migrate remaining services** one by one; keep existing tests green and add schema-source tests.
4. **Version and profile:** Add config/profile for schema version and strictness once the pipeline is stable.

## Summary

| Area              | Current                         | Target                                      |
|-------------------|---------------------------------|---------------------------------------------|
| Schema source     | Rust types → `schemars`         | GA4GH official OpenAPI/JSON Schema files    |
| Loading           | Compile-time via macro          | Load (and optionally compile) at runtime    |
| Validation        | `validate_json_against<T>`      | Validate against loaded official schema     |
| Versioning        | Implicit (our structs)          | Explicit (e.g. WES 1.0 / 1.1, configurable) |
| Coverage          | Subset we model                 | Full spec components we choose to validate |

This keeps the existing test flow (run requests, assert on responses) and only changes *how* response structure is validated (spec-driven instead of type-driven).
