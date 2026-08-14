# Changelog

## Unreleased

- Scoring: **Skip** is not a pass (excluded from levels, scores, `--fail-level`). Empty/skip-only levels do not block higher N.
- HTTP client: shorter timeouts, GET retries only, no POST retry.
- Single workspace at repo root; crates published as `helixtest-common` / `helixtest-framework`.
- `--start-ferrum` polls WES; optional `--compose-file`.
- TES checksums from `outputs[].url` or a **fresh** local file; E2E requires a golden checksum; no TES id == WES `run_id`.
- Auth L0 hits `auth_url`; local crypto tests are **age**; Passports are `ferrum+infra`.
- Remaining gaps: [known-limitations.md](known-limitations.md).
- Docs: [ADR 0001](adr/0001-ferrum-tes-ci-vs-docker-stack-and-db-init.md) — Ferrum **noop TES (CI / HelixTest defaults)** vs **Docker TES (demos)**; TES env reference; **`ferrum-init` / Postgres** reset expectations. Linked from [ferrum.md](ferrum.md) and READMEs.

- Add first-class subset conformance support:
  - profile-driven enabled/disabled service gating
  - explicit report metadata for enabled/skipped/executed modules
  - token-only auth checks (`token-protected-endpoints`) for protected API endpoints
  - new `bioresearch-assistant` profile and subset-profile documentation
- Clarify **conformance vs performance**: lifecycle polling docs, WES success path allows `QUEUED`/`INITIALIZING`/`RUNNING` before `COMPLETE`, TES poll comments, framework E2E scope vs `e2e-tests`; new `docs/conformance-philosophy.md`.
- Crypt4GH: **truncated ciphertext** robustness check (local Level 5).
- Optional JSON **`diagnostics`** (`suite_duration_ms`) when `HELIXTEST_REPORT_DIAGNOSTICS` is set — **not** used for levels/scores.
- Operator **resource hints** in `docs/subset-profiles.md` and `docs/ferrum.md`.
