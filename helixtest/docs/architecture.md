# HelixTest Architecture

This document describes the high-level architecture of the HelixTest GA4GH conformance suite.

## Overview

HelixTest is a Rust workspace that runs conformance tests against GA4GH-compliant services. The **CLI** orchestrates a **framework** that executes per-service checks and E2E pipelines; shared logic lives in **common**. Configuration and profiles drive endpoints and feature flags.

**Cross-repo / operator notes** (e.g. Ferrum noop vs Docker TES, DB init): see [ADR index](adr/).

## Architecture Diagram

```
┌─────────────────────────────────────────────────────────────────────────┐
│  User / CI                                                               │
│  helixtest --all [--report table|json|scores|coverage]                  │
└─────────────────────────────────────────────────────────────────────────┘
                                      │
                                      ▼
┌─────────────────────────────────────────────────────────────────────────┐
│  CLI (crates/cli)                                                        │
│  • Parses args (--all, --mode, --report, --fail-level, --only, --verbose)│
│  • Loads config (HELIXTEST_PROFILE / HELIXTEST_CONFIG / env)            │
│  • Calls framework::run_all(mode)                                       │
│  • Renders report (table, JSON, scores, coverage) and sets exit code      │
└─────────────────────────────────────────────────────────────────────────┘
                                      │
                                      ▼
┌─────────────────────────────────────────────────────────────────────────┐
│  Framework (crates/framework)                                             │
│  • Resolves effective mode (generic vs Ferrum, auto-detect from WES)     │
│  • Loads features from profiles/<profile>.toml or ferrum.toml             │
│  • Runs per-service checks in sequence:                                   │
│    WES → DRS → TRS → TES → Beacon → htsget → Auth → Crypt4GH → E2E       │
│  • Returns OverallReport { services: Vec<ServiceReport> }                │
└─────────────────────────────────────────────────────────────────────────┘
          │                │                │                │
          ▼                ▼                ▼                ▼
┌──────────────┐  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐
│ wes.rs       │  │ drs.rs       │  │ trs.rs       │  │ tes.rs       │
│ beacon.rs    │  │ htsget.rs    │  │ auth.rs      │  │ crypt4gh.rs  │
│ e2e.rs       │  │              │  │              │  │              │
└──────────────┘  └──────────────┘  └──────────────┘  └──────────────┘
          │                │                │                │
          └────────────────┴────────────────┴────────────────┘
                                      │
                                      ▼
┌─────────────────────────────────────────────────────────────────────────┐
│  Common (crates/common)                                                  │
│  • config    – TestConfig, ServiceConfig, from_env_or_file, profiles     │
│  • http      – HttpClient (timeout, retry), get_json, post_json         │
│  • workflow  – WES submit/poll/fetch, WesRunRequest                      │
│  • report    – ComplianceLevel, TestCategory, TestCaseResult,           │
│                ServiceReport, OverallReport, to_table, score_summary,     │
│                coverage_summary                                          │
│  • auth      – JWT build (HMAC-SHA256) for Auth checks                   │
│  • crypto    – age encrypt/decrypt for Crypt4GH checks                   │
│  • logging   – tracing init with RUST_LOG                                │
│  • schemas   – validate_json_against<T> (jsonschema)                      │
└─────────────────────────────────────────────────────────────────────────┘
                                      │
                                      ▼
┌─────────────────────────────────────────────────────────────────────────┐
│  External services (configured via [services] or env)                    │
│  WES, TES, DRS, TRS, Beacon, Auth                                       │
└─────────────────────────────────────────────────────────────────────────┘
```

## Data Flow

1. **Config** – Endpoints come from `HELIXTEST_PROFILE` → `profiles/<name>.toml`, or `HELIXTEST_CONFIG`, or `helixtest-config.toml`, or `WES_URL`/… environment variables.
2. **Features** – Framework loads `[features]` from the same profile (or Ferrum mode) to enable/disable tests (e.g. `supports_beacon_v2`, `strict_drs_checksums`).
3. **Execution** – Each service module (e.g. `wes.rs`) returns a `ServiceReport` with a list of `TestCaseResult` (name, level, passed, error, category, weight).
4. **Aggregation** – `OverallReport` aggregates all services; the CLI can filter by `--only` and then render table/JSON/scores/coverage.
5. **Exit code** – CLI exits 1 if any test failed or if `--fail-level N` is set and overall level is below N.

## Test Crates vs Framework

- **Framework** (`crates/framework`) – Conformance runs used by the CLI: one entrypoint `run_all()`, produces `OverallReport`. This is what `helixtest --all` runs.
- **api-tests, workflow-tests, e2e-tests, auth-tests, crypt4gh-tests** – Separate crates with `#[test]`/`#[tokio::test]` for development and CI; they reuse `common` and may run against the same config but are not invoked by the default `--all` flow.

## Key Types

| Type | Location | Purpose |
|------|----------|---------|
| `TestConfig` | common::config | Service URLs and profile path |
| `Features` | framework | Feature flags (beacon, checksums, scatter/gather) |
| `TestCaseResult` | common::report | Single test: name, level, passed, error, category, weight |
| `ServiceReport` | common::report | One service: list of TestCaseResult, achieved_level, weighted_score |
| `OverallReport` | common::report | All services; to_table, score_summary, coverage_summary |

---

*HelixTest by Synaptic Four — built for the open science community. © 2025 Synaptic Four · Apache-2.0.*
