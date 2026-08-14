# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- `TestStatus` (Pass / Fail / Skip); skips are excluded from levels, scores, and `--fail-level`.
- `--compose-file` and WES `/service-info` health poll for `--start-ferrum`.
- Workspace `rust-version = "1.85"`; packages `helixtest-common` / `helixtest-framework`.
- Separate **Age** service vs env-gated **Crypt4GH** HTTP; `--only age`.
- CI MSRV job (`cargo check --locked` on 1.85); committed `Cargo.lock`.

### Changed

- Single Cargo workspace at repo root; nested `helixtest/Cargo.toml` workspace removed.
- HTTP: 5s connect / 30s request; GET retried twice; POST not retried.
- CLI loads profile via `TestConfig::load` instead of mutating process env.
- Local “Crypt4GH” suite is a separate **Age** service; HMAC JWT requires `HELIXTEST_SHARED_SECRET` (no `test-secret` default).
- Achieved level requires executed Level 0; L5-only suites report Level 0.
- jsonschema 0.17 still leak-once per official schema; `validate_json_against` removed.
- Conformance runner no longer sets AVX2 / `target-cpu` rustflags.

### Fixed

- Skip results no longer count as passes; `weight <= 0` omitted from scores.
- TES/E2E checksums no longer green on stale local files or missing goldens.
- WES timeout-robustness no longer fails a fast stack; scatter/gather gated on `supports_scatter_gather`.
- L0 reachability requires 2xx or 401 (not any 4xx).
- Africa federation uses `FERRUM_AFRICA_PEER_URL`; Auth L0 uses `auth_url`.

See [helixtest/docs/known-limitations.md](helixtest/docs/known-limitations.md) for remaining constraints (serial HTTP, live-stack cargo tests excluded from CI).

### Security
