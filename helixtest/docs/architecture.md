# HelixTest Architecture

This document describes the high-level architecture of the HelixTest GA4GH conformance suite.

## Overview

HelixTest is a Rust workspace that runs conformance tests against GA4GH-compliant services. The **CLI** orchestrates a **framework** that executes per-service checks and E2E pipelines; shared logic lives in **common**. Configuration and profiles drive endpoints and feature flags.

Cargo packages are named `helixtest-common` and `helixtest-framework` (Rust crate names remain `common` / `framework`). There is a **single workspace** at the repository root (`HelixTest/Cargo.toml`). Run `cargo` from that root.

**Cross-repo / operator notes** (e.g. Ferrum noop vs Docker TES, DB init): see [ADR index](adr/). Remaining product gaps: [known-limitations.md](known-limitations.md).

## Architecture Diagram

```
┌─────────────────────────────────────────────────────────────────────────┐
│  User / CI                                                               │
│  helixtest --all [--report table|json|scores|coverage]                  │
└─────────────────────────────────────────────────────────────────────────┘
                                      │
                                      ▼
┌─────────────────────────────────────────────────────────────────────────┐
│  CLI (crates/cli, package helixtest-cli)                                 │
│  • Parses args (--all, --mode, --profile, --report, --fail-level,        │
│    --only, --verbose, --start-ferrum, --compose-file)                    │
│  • Loads config via TestConfig::load(profile) (no process env mutation)  │
│  • Calls framework::run_all(mode, only, profile)                         │
│  • Renders report (table, JSON, scores, coverage) and sets exit code      │
└─────────────────────────────────────────────────────────────────────────┘
                                      │
                                      ▼
┌─────────────────────────────────────────────────────────────────────────┐
│  Framework (crates/framework, package helixtest-framework)               │
│  • Resolves effective mode (generic vs Ferrum, auto-detect from WES)     │
│  • Loads features from profiles/<profile>.toml or ferrum.toml             │
│  • Runs per-service checks in canonical order:                            │
│    WES → TES → DRS → TRS → Beacon → htsget → Auth → Age → Crypt4GH → E2E │
│  • Returns OverallReport { services: Vec<ServiceReport> }                │
└─────────────────────────────────────────────────────────────────────────┘
          │                │                │                │
          ▼                ▼                ▼                ▼
┌──────────────┐  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐
│ wes.rs       │  │ tes.rs       │  │ drs.rs       │  │ trs.rs       │
│ beacon.rs    │  │ htsget.rs    │  │ auth.rs      │  │ crypt4gh.rs  │
│ e2e.rs       │  │ africa.rs    │  │ infra.rs     │  │              │
└──────────────┘  └──────────────┘  └──────────────┘  └──────────────┘
          │                │                │                │
          └────────────────┴────────────────┴────────────────┘
                                      │
                                      ▼
┌─────────────────────────────────────────────────────────────────────────┐
│  Common (crates/common, package helixtest-common)                        │
│  • config    – TestConfig::load(profile), profiles under helixtest/      │
│  • http      – HttpClient (5s connect / 30s request); GET retried twice; │
│                POST is not retried                                       │
│  • workflow  – WES submit/poll/fetch, WesRunRequest                      │
│  • report    – TestStatus, ComplianceLevel, TestCaseResult,              │
│                ServiceReport, OverallReport, to_table, score_summary,     │
│                coverage_summary                                          │
│  • auth      – HMAC-SHA256 JWT fixture (not Passports)                   │
│  • crypto    – age encrypt/decrypt for local “Crypt4GH-style” checks     │
│  • logging   – tracing init with RUST_LOG / --verbose                    │
│  • schemas   – assert_required_string_field; ga4gh_schemas (OpenAPI)     │
└─────────────────────────────────────────────────────────────────────────┘
                                      │
                                      ▼
┌─────────────────────────────────────────────────────────────────────────┐
│  External services (configured via [services] or env)                    │
│  WES, TES, DRS, TRS, Beacon, Auth, htsget                               │
└─────────────────────────────────────────────────────────────────────────┘
```

## Data Flow

1. **Config** – Endpoints come from `--profile` / `HELIXTEST_PROFILE` → `helixtest/profiles/<name>.toml`, or `HELIXTEST_CONFIG`, or `helixtest-config.toml`, or `WES_URL`/… environment variables. The CLI does not `set_var` the profile.
2. **Features** – Framework loads `[features]` from the same profile (or Ferrum mode) to enable/disable tests (e.g. `supports_beacon_v2`, `strict_drs_checksums`, `supports_scatter_gather`). Missing/invalid profiles are errors.
3. **Execution** – Each service module (e.g. `wes.rs`) returns a `ServiceReport` with `TestCaseResult` values (`status` Pass/Fail/Skip, level, category, weight).
4. **Aggregation** – `OverallReport` aggregates all services; the CLI can filter by `--only` and then render table/JSON/scores/coverage. Canonical order is WES, TES, DRS, TRS, Beacon, htsget, Auth, Age, Crypt4GH, E2E.
5. **Exit code** – CLI exits 1 if any test **Failed** or if `--fail-level N` is set and overall level is below N. Skips are not failures.

## Test Crates vs Framework

- **Framework** (`crates/framework`) – Conformance runs used by the CLI: `run_all(mode, only, profile)`, produces `OverallReport`. This is what `helixtest --all` runs.
- **api-tests, workflow-tests, e2e-tests, auth-tests, crypt4gh-tests** – Separate crates with `#[test]`/`#[tokio::test]` for development and CI; they reuse `common` and may run against the same config but are not invoked by the default `--all` flow. They are excluded from default CI `cargo test --workspace`.

## Key Types

| Type | Location | Purpose |
|------|----------|---------|
| `TestConfig` | common::config | Service URLs; `load(profile)` |
| `Features` | framework | Feature flags (beacon, checksums, scatter/gather) |
| `TestStatus` | common::report | Pass, Fail, Skip |
| `TestCaseResult` | common::report | Single test: name, level, status, passed, error, category, weight |
| `ServiceReport` | common::report | One service: list of TestCaseResult, achieved_level, weighted_score |
| `OverallReport` | common::report | All services; to_table, score_summary, coverage_summary |

---

*HelixTest by Synaptic Four — built for the open science community. © 2025 Synaptic Four · Apache-2.0.*
