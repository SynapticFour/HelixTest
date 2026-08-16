# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

- Live GHCR demo: `sudo install -d` for the SQLite bind mount (chmod after chown 65532 failed as the runner). Seed step opens the mount for host `sqlite3`, then chowns back to 65532.
- Live GHCR auth-on runs `helixtest --only auth` (HMAC on DRS `service-info` via `HELIXTEST_AUTH_SURFACE=service-info`). Curl HS256 remains a pre-check. Not Passport/AAI.
- HMAC auth suite: `HELIXTEST_AUTH_SURFACE=service-info` for published edge (no `test-object-1`); garbage Bearer still must be 401.

## [0.1.2] - 2026-08-16

Ferrum CI should pin this tag (`VERSIONS.lock` `HELIXTEST_REF=v0.1.2`). `v0.1.1` remains the previous cut.

### Changed

- `make test` / `make prove` use the same offline crate excludes as CI (`api-tests`, `auth-tests`, `e2e-tests`, `workflow-tests` need a live target).
- CI `build-and-test` and ARM jobs run `make prove` (same command as a local clone).
- Live Ferrum GHCR default image is `ghcr.io/synapticfour/ferrum:v0.3.1-edge` (schedule uses the same fallback). GitHub Release ships linux-gnu x86_64/aarch64 and darwin aarch64 binaries.
- `--mode ferrum+infra`: unreachable broker/registry/login **fails** (no skip-as-green). Profile `ferrum-infra-pilot` matches Ferrum `make up-pilot-local` on port 8080.
- **Live Ferrum GHCR auth-on** workflow: published `v0.3.1-edge` with `require_auth=true` (HS256). Demo Live GHCR job stays auth-off.
- Third-party wrapper: [synapticfour/helixtest-action](https://github.com/SynapticFour/helixtest-action) (v0.1.1 binaries until this tag’s release assets exist). Does not start Ferrum.
- **SPDX on first-party `.rs`** — `// SPDX-License-Identifier: Apache-2.0`; CI `spdx.yml`.
- **DRS + Beacon official schemas** — Level 1 validates DRS `DrsObject` (OpenAPI 1.4.0) and Beacon v2 `beaconBooleanResponse` (bundled draft-07). Not Ferrum’s utoipa dump.
- Gitleaks allowlists example JWTs in the vendored DRS OpenAPI (spec fixtures, not credentials).

## [0.1.1] - 2026-08-15

HelixTest `v0.1.0` remains on origin for the earlier cut. This tag is what Ferrum CI pins (`VERSIONS.lock` `HELIXTEST_REF=v0.1.1`).

### Added

- `TestStatus` (Pass / Fail / Skip); skips are excluded from levels, scores, and `--fail-level`.
- `--compose-file` and WES `/service-info` health poll for `--start-ferrum`.
- Workspace `rust-version = "1.88"`; packages `helixtest-common` / `helixtest-framework`.
- Separate **Age** service vs env-gated **Crypt4GH** HTTP; `--only age`.
- CI MSRV job (`cargo check --locked` on 1.88); committed `Cargo.lock`.

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
- **htsget POST “no query”** sent `regions` in the JSON body (a region-slice request). Those checks now POST `{format}` only; separate tests cover `regions`.
- **htsget POST `regions` (Ferrum / FerrumAfrica / FerrumInfra)** — expect **HTTP 400** `InvalidInput`. Ferrum does not slice BAM/VCF; a silent whole-file ticket is not treated as a pass. Generic mode still accepts a 2xx whole-file ticket.
- Africa and E2E include Level 0 reachability so `--fail-level 1` can pass when every executed test passed (L2-only / L3-only suites previously scored as 0).

See [helixtest/docs/known-limitations.md](helixtest/docs/known-limitations.md) for remaining constraints (serial HTTP, live-stack cargo tests excluded from CI).

## [0.1.0] - 2026-08-01

First tagged HelixTest cut on origin (`v0.1.0`). See git history for notes prior to this file’s Keep a Changelog sections.
