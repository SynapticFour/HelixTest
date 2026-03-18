# Contributing to HelixTest

Thank you for considering contributing to the HelixTest GA4GH conformance suite. This guide explains how to add new tests and extend the framework.

## How to Add New Tests

### 1. Conformance tests (run by `helixtest --all`)

Tests that appear in the main suite live in **`crates/framework/src/`**, one module per service (or E2E).

**Add a new test case:**

1. **Choose the right module** – e.g. `wes.rs`, `drs.rs`, `auth.rs`, `e2e.rs`.
2. **Implement an `async fn` that returns `TestCaseResult`**:
   - Use `TestConfig` and `HttpClient` (or other helpers from `common`).
   - Call the service, assert on response.
   - Return:
     ```rust
     TestCaseResult {
         name: "Short human-readable name".into(),
         level: ComplianceLevel::LevelN,  // 0–5
         passed: result.is_ok(),
         error: result.err().map(|e| e.to_string()),
         category: TestCategory::Schema,  // or Lifecycle, Checksum, etc.
         weight: 1.0,
     }
     ```
3. **Register the test** in the service’s `run_*_checks` function by pushing the result of your new function onto the `tests` vector.
4. **Rebuild and run** – `cargo run --bin helixtest -- --all` will include your test.

**Example (skeleton):**

```rust
async fn level2_my_new_check(cfg: &TestConfig, client: &HttpClient) -> TestCaseResult {
    let result = async {
        let url = format!("{}/some-path", cfg.services.wes_url.trim_end_matches('/'));
        let v = client.get_json(&url).await?;
        // ... validate v ...
        Ok::<(), anyhow::Error>(())
    }.await;
    TestCaseResult {
        name: "WES my new check".into(),
        level: ComplianceLevel::Level2,
        passed: result.is_ok(),
        error: result.err().map(|e| e.to_string()),
        category: TestCategory::Lifecycle,
        weight: 1.0,
    }
}
```

Then in `run_wes_checks`:

```rust
tests.push(level2_my_new_check(cfg, client).await);
```

### 2. Standalone test crates (e.g. api-tests, e2e-tests)

- These use `#[tokio::test]` or `#[test]` and live under `crates/api-tests`, `crates/e2e-tests`, etc.
- They are run with `cargo test -p api-tests` (or the relevant crate), not by `--all`.
- To add a test there, add a new test function in the crate’s `tests` module and follow the same patterns (config, client, assertions).

### 3. Categories and levels

- **Level** – Use the table in [scoring.md](scoring.md): 0 = reachable, 1 = schema, 2 = functional, 3 = interoperability, 4 = security, 5 = robustness.
- **Category** – Use `TestCategory`: Schema, Lifecycle, WorkflowCorrectness, Checksum, Interoperability, Security, Robustness, Other. This drives `--report coverage`.

## Adding a New Service

1. **Add a new module** in `crates/framework/src/`, e.g. `myservice.rs`.
2. **Implement `pub async fn run_myservice_checks(...) -> Result<ServiceReport>`** – same signature pattern as `run_wes_checks`.
3. **Add `ServiceKind::Myservice`** in `crates/common/src/report.rs` and implement `Display`.
4. **Call your runner** from `framework::run_all()` in `lib.rs` and push the report onto `services`.
5. **Optional:** extend `TestConfig` / `ServiceConfig` and profiles if the service needs its own URL.

## Profiles and feature flags

- **Endpoints** – Add or reuse `[services]` in `profiles/generic.toml`, `profiles/ferrum.toml`, `profiles/strict.toml`.
- **Feature flags** – Extend `Features` in `crates/framework/src/lib.rs` and use it in the relevant tests (e.g. to skip or enable a check). Document the flag in the profile TOML and in the README.

## Code style and quality

- Use existing patterns (async, `anyhow::Result`, `tracing` for logging).
- Prefer clear error messages in `TestCaseResult.error` so users see why a test failed.
- Run `cargo build -p helixtest-cli` and `cargo test` (for the crates you touch) before submitting.

## References

- [Architecture](architecture.md) – high-level layout and data flow.
- [Scoring](scoring.md) – levels, scores, and coverage.
- [Disclaimer](DISCLAIMER.md) – limitation of liability and use of test results.
- [GA4GH official schemas](ga4gh-official-schemas.md) – outline for using official OpenAPI/JSON schemas instead of Rust-derived schemas.
- GA4GH specifications for WES, TES, DRS, TRS, Beacon, Passports, etc., when implementing schema or behavioral checks.

## Before making the project public

- [x] **Legal:** LICENSE and NOTICE are in repo root; README and docs reference [DISCLAIMER](DISCLAIMER.md) and state that test results are not official certification.
- [x] **Wording:** No unconditional “guarantee” or “certified”; use “conformance checks,” “for informational use,” “as is” where appropriate.
- [x] **CI:** `cargo build --bin helixtest` and `cargo test --workspace` pass.
- [x] **Config:** Default config or env fallbacks are documented; no secrets or internal URLs in repo.
- [x] **Attribution:** Synaptic Four USP, copyright (© 2025), and contact (contact@synapticfour.com · synapticfour.com) appear in README, NOTICE, and CLI (banner/help).

---

*HelixTest by Synaptic Four — built for the open science community. © 2025 Synaptic Four · Apache-2.0.*
