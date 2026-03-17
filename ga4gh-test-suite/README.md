## HelixTest – GA4GH Conformance & Interoperability Suite

**Run conformance checks against GA4GH systems.**

HelixTest is a Rust-based conformance framework for GA4GH APIs and workflow platforms. It focuses on strict validation, cross-service interoperability, and security/robustness. It is **CI-ready** (exit codes, JSON reports, `--fail-level`), **usable by any GA4GH-compliant platform** (config-driven endpoints, profiles), and suitable as a **reference conformance suite** for the GA4GH ecosystem: test cases and compliance levels align with GA4GH service specifications and can be used as a reference when building or validating compliant implementations.

HelixTest currently targets:

- **WES** – Workflow Execution Service
- **TES** – Task Execution Service
- **DRS** – Data Repository Service
- **TRS** – Tool Registry Service
- **Beacon v2**
- **GA4GH Passports / AAI / OIDC**
- **Crypt4GH-style encryption** (using `age` as a pluggable backend)

For Ferrum-specific guidance, see the dedicated document: [docs/ferrum.md](docs/ferrum.md).

---

### Architecture

HelixTest is organized as a **CLI** that runs a **framework** of per-service checks and E2E tests; shared logic lives in **common** (config, HTTP, reporting). Configuration and profiles supply endpoints and feature flags.

```
  User: ga4gh-test-suite --all [--report table|json|scores|coverage]
                    │
                    ▼
  CLI ──────────► Framework (WES, DRS, TRS, TES, Beacon, Auth, Crypt4GH, E2E)
                    │
                    ▼
  Common (config, http, workflow, report, auth, crypto, logging, schemas)
                    │
                    ▼
  External services (WES, TES, DRS, TRS, Beacon, Auth)
```

See **[docs/architecture.md](docs/architecture.md)** for a full architecture diagram and data flow.

---

### Workspace Layout

- `crates/common` – shared config, HTTP client, polling, logging, schemas, reporting.
- `crates/framework` – compliance levels, per-service scoring, orchestration.
- `crates/api-tests` – per-API contract tests (WES, TES, DRS, TRS, Beacon).
- `crates/workflow-tests` – workflow-level WES tests (CWL/WDL/Nextflow, scatter/gather).
- `crates/e2e-tests` – full TRS → DRS → WES → TES → Beacon pipelines.
- `crates/auth-tests` – GA4GH Passports / OIDC security tests.
- `crates/crypt4gh-tests` – encryption/decryption, corruption, wrong-key, streaming tests.
- `crates/cli` – `ga4gh-test-suite` CLI.
- `test-data/` – workflows, inputs, deterministic outputs, and `.sha256` checksums.
- `profiles/` – profiles such as `ferrum.toml` with feature flags.
- `docker/` – optional `docker-compose.yml` for local/mock environments.

---

### GA4GH Compliance Levels

HelixTest uses a 0–5 compliance ladder per service:

- **Level 0** – API reachable
- **Level 1** – Schema compliant (structure, required fields, enums)
- **Level 2** – Functional correctness (lifecycle, outputs, checksums)
- **Level 3** – Interoperability (cross-service flows)
- **Level 4** – Security compliance (auth, scopes, expiry)
- **Level 5** – Robustness (negative cases, corruption, wrong keys, etc.)

Each test case is tagged with a `ComplianceLevel` and contributes to per-service and overall scores. The framework computes:

- **Per-service achieved level** – The highest level N such that every test at level N for that service passed.
- **Overall level** – The minimum of all per-service achieved levels.
- **Weighted score** – Per service and overall, a value in [0.0, 1.0] based on test weights (1.0 = all passed).

Use `--report scores` for a JSON summary of levels and scores, and `--fail-level N` to exit with code 1 if overall level is below N.

For full details (levels, scores, coverage matrix), see **[docs/scoring.md](docs/scoring.md)**.

---

### Configuration

Configuration is loaded from a TOML file or environment variables.

#### TOML config (recommended)

Create `ga4gh-test-config.toml` in the `ga4gh-test-suite` directory:

```toml
[services]
wes = "http://localhost:8080"
tes = "http://localhost:8081"
drs = "http://localhost:8082"
trs = "http://localhost:8083"
beacon = "http://localhost:8084"
auth = "http://localhost:8085"
```

You can point to a custom config file with:

```bash
export GA4GH_TEST_CONFIG=/path/to/ga4gh-test-config.toml
```

#### Environment variable fallback

If no TOML config is found, HelixTest uses:

- `WES_URL`
- `TES_URL`
- `DRS_URL`
- `TRS_URL`
- `BEACON_URL`
- `AUTH_URL`

These must point at your GA4GH-compliant deployment.

#### For platform implementers

If you maintain a GA4GH-compliant platform (WES, TES, DRS, TRS, Beacon, Auth), you can run HelixTest against your deployment to validate conformance:

1. **Point HelixTest at your services** – Set the base URLs via a config file (`ga4gh-test-config.toml` or `GA4GH_TEST_CONFIG`) or environment variables (`WES_URL`, `TES_URL`, etc.).
2. **Run the suite** – `cargo run --bin ga4gh-test-suite -- --all` (optionally `--report json` or `--report scores` for machine-readable output).
3. **Gate releases or PRs** – Use `--fail-level N` (e.g. `2`) so the process exits non-zero if overall compliance is below that level; integrate this into your CI (see **CI integration** below).

HelixTest is **usable by any GA4GH-compliant platform** with no code changes; configuration is entirely endpoint-driven.

---

### Running the Suite

From the `ga4gh-test-suite` directory:

```bash
cargo run --bin ga4gh-test-suite -- --all
```

**Options:** `--report table|json|scores|coverage` (default: table), `--mode generic|ferrum`, `--start-ferrum`, `--fail-level <N>`, `--only <service>` (repeatable), `--verbose`.

**Examples:**

```bash
cargo run --bin ga4gh-test-suite -- --all --report table
cargo run --bin ga4gh-test-suite -- --all --report json > helix-report.json
cargo run --bin ga4gh-test-suite -- --all --report scores
cargo run --bin ga4gh-test-suite -- --all --fail-level 3
cargo run --bin ga4gh-test-suite -- --all --mode ferrum --start-ferrum
```

**Exit codes:** `0` – all tests passed (and overall level ≥ `--fail-level` if set). `1` – at least one test failed or level below `--fail-level`.

#### CI integration

HelixTest is designed for CI pipelines:

- **Exit codes:** Use as a direct step; non-zero on failure or when `--fail-level` is not met.
- **Structured output:** Use `--report json` or `--report scores` for machine-readable results; `--report table` for human-readable logs.
- **Deterministic results:** Table and JSON output are ordered consistently (by service) for stable diffs and caching.
- **Logging:** Set `RUST_LOG=info` (or `debug` with `--verbose`); log lines include structured key=value fields.
- **Errors:** Failed tests and startup failures produce clear, actionable messages (e.g. missing config, timeout, schema violation).

**Example (GitHub Actions–style):**

```bash
cargo run --bin ga4gh-test-suite -- --all --report json --fail-level 2 > report.json
echo $?  # 0 = pass, 1 = fail
```

A minimal **GitHub Actions** workflow (build + test) is provided in the repo root: [.github/workflows/conformance.yml](../.github/workflows/conformance.yml). To run conformance against your deployment in CI, add a job that sets `WES_URL`, `TES_URL`, etc. (e.g. from repository secrets) and runs the command above.

**Example table output:**

```
Service   Level   Details
=======   =====   =======
WES       2       OK
TES       2       OK
DRS       2       OK
...
```

#### How to add new tests

New tests are added in the framework (`crates/framework`) and tagged with a compliance level and category. For step-by-step instructions (defining test cases, categories, levels, new services, and profiles), see **[docs/contributing.md](docs/contributing.md)**.

---

### What HelixTest Validates

- **WES**
  - `/service-info` schema and supported versions.
  - Lifecycle: `QUEUED → INITIALIZING → RUNNING → TERMINAL`, no invalid transitions.
  - Success workflows (CWL/WDL/NFL echo) with deterministic outputs and checksums.
  - Failure workflows, invalid descriptors, missing inputs, incompatible `workflow_type` → must end in error states.

- **TES**
  - `/tasks` reachability.
  - Task JSON schema (create + status).
  - Lifecycle to `COMPLETE` + deterministic output checksum.

- **DRS**
  - Core fields (`id`, `self_uri`, `name`, `access_methods`).
  - Checksum correctness: SHA256 from `checksums` vs SHA256 computed from **HTTP download via `access_url`**.
  - HTTP Range: `206` with valid `Content-Range` for `bytes=0-1023`.
  - Invalid ID returns `404`.

- **TRS**
  - `/tools` and `/tools/{id}/versions` schema.
  - Descriptor retrieval for at least one tool/version.
  - TRS registry host for `trs://` URLs derived from `TRS_URL` (no hardcoded registry).

- **Beacon v2**
  - `/query` reachability and response schema.
  - Known variant must have `response.exists == true`.
  - Negative variant must have `response.exists == false`.
  - Can be feature-gated via `supports_beacon_v2` (see `profiles/ferrum.toml`).

- **Auth / GA4GH Passports**
  - Valid token + scope → success.
  - Missing token → `401`.
  - Expired token → `401`.
  - Invalid signature → `401`.
  - Wrong scope → `403`.

- **Crypt4GH-style encryption**
  - Round-trip checksum equality.
  - Partial decrypt (prefix bytes).
  - Corrupted header / ciphertext must fail to decrypt.
  - Wrong key must fail.
  - Streaming-compatible reads preserve checksum.

- **E2E Interoperability**
  - TRS → DRS → WES → TES → DRS → Beacon pipeline:
    - Tool/version discovery in TRS.
    - DRS inputs/outputs with ID propagation.
    - WES run execution with correct lifecycle and outputs.
    - TES tasks linked to WES run IDs.
    - Output object downloaded from DRS via `access_url` and validated via checksum.
    - Beacon reports presence of a known variant after pipeline execution.

---

### Ferrum Integration

HelixTest has first-class support for testing **Ferrum**, a Rust-based GA4GH platform:

- Mode selection: `--mode ferrum`.
- Optional auto-start via Docker: `--start-ferrum`.
- Feature flags via `profiles/ferrum.toml` to:
  - Enable/disable scatter/gather workflow checks.
  - Enable/disable Beacon v2 tests.
  - Enable/disable strict DRS checksum checks.

See `docs/ferrum.md` for a step-by-step Ferrum testing guide.

---

### Reporting

- **Table output** (default):
  - Shows each service, achieved compliance level, and any failing tests with reasons.
- **JSON output** (`--json`):
  - Emits a full `OverallReport` structure with:
    - Per-service test results (names, levels, pass/fail, error messages).
    - Ideal for CI pipelines, dashboards, and automated analysis.

---

### Contributing

Contributions are welcome, especially around:

- New GA4GH standards or profiles.
- Additional workflow scenarios and negative tests.
- Improved schema coverage using official GA4GH OpenAPI/JSON schemas.
- Additional profiles (cloud, HPC, vendor-specific).

Please include clear descriptions and, where possible, references to GA4GH specifications when submitting changes.

## HelixTest (Rust workspace)

This workspace provides **HelixTest**, a GA4GH conformance and integration test suite for systems implementing:

- **WES** (Workflow Execution Service)
- **TES** (Task Execution Service)
- **DRS** (Data Repository Service)
- **TRS** (Tool Registry Service)
- **Beacon v2**
- **GA4GH Passports / AAI / OIDC**
- **Crypt4GH-style** encryption (pluggable, `age`-based reference implementation)

### Workspace Layout

- `crates/common` – shared config, HTTP client, polling, logging, domain types
- `crates/api-tests` – per-API contract tests with JSON-schema-style validation
- `crates/workflow-tests` – workflow-level WES tests (CWL/WDL/Nextflow)
- `crates/e2e-tests` – full cross-service integration tests
- `crates/auth-tests` – GA4GH Passports / OIDC tests
- `crates/crypt4gh-tests` – encryption / decryption and integrity tests
- `crates/cli` – **HelixTest** CLI (`ga4gh-test-suite` binary) for orchestrating the above
- `test-data/` – workflows, inputs, and expected outputs
- `docker/` – `docker-compose.yml` with mock GA4GH services

### Running Tests

1. Start mock services:

```bash
cd docker
docker compose up -d
```

2. Run all tests via CLI:

```bash
cargo run --bin ga4gh-test-suite -- --all
```

3. Or run specific test crates:

```bash
cargo test -p api-tests
cargo test -p workflow-tests
cargo test -p e2e-tests
cargo test -p auth-tests
cargo test -p crypt4gh-tests
```

### Configuration

Configuration is loaded from environment variables or an optional TOML config file.

Supported environment variables (examples):

- `WES_URL=https://wes.example.org`
- `TES_URL=https://tes.example.org`
- `DRS_URL=https://drs.example.org`
- `TRS_URL=https://trs.example.org`
- `BEACON_URL=https://beacon.example.org`
- `AUTH_URL=https://auth.example.org`
- `GA4GH_TEST_CONFIG=./ga4gh-test-config.toml` (optional config file path)

See `crates/common/src/config.rs` for details.

---

### Disclaimer and limitation of liability

HelixTest is provided **as is**, without warranty of any kind, express or implied. Test results and reported levels are for informational and development use only; they do **not** constitute official GA4GH certification or a guarantee of conformance. You use this software at your own risk. See [docs/DISCLAIMER.md](docs/DISCLAIMER.md) and the [LICENSE](../LICENSE) (Apache-2.0) for full terms.

---

**Synaptic Four** — Built with ❤️ for the open science community. Implementing GA4GH open standards for sovereign bioinformatics infrastructure. Proudly developed by individuals on the autism spectrum in Germany 🇩🇪 We build tools that are precise, thorough, and designed to work exactly as documented.  
© 2025 Synaptic Four · Licensed under [Apache-2.0](../LICENSE).

