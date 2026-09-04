# Testing

HelixTest is a runner you point at a **target you started**. Results are a technical signal, **not GA4GH certification**. Ferrum is a reference target, not a dependency. This file describes **what this repository actually runs**. It is not a vision doc and not HELIOS (no RO-Crate / signed evidence).

Workspace members (`Cargo.toml`): `helixtest-common`, `helixtest-framework`, `api-tests`, `workflow-tests`, `e2e-tests`, `auth-tests`, `crypt4gh-tests`, `helixtest-cli` (`helixtest` binary).

`make prove` / `make test` use:

```text
cargo test --workspace --exclude api-tests --exclude auth-tests --exclude e2e-tests --exclude workflow-tests
```

That is the same exclude list as `.github/workflows/conformance.yml` and `scripts/hooks/ci-check.sh`.

---

## 1. Was läuft wo

| Testart | Crate / Workflow | Offline (`make prove`) oder live target | Trigger |
|---------|------------------|-----------------------------------------|---------|
| Offline workspace tests: `helixtest-common`, `helixtest-framework` (incl. in-process mock DRS `framework/tests/generic_drs_mock.rs`), `crypt4gh-tests` (local **age** roundtrip, not Crypt4GH HTTP), `helixtest-cli` (`cli_smoke.rs` + `generic_drs_independence.rs` against `helixtest/testing/mock_ga4gh_drs.rs`) | `make test` / `make prove` | **Offline** (wiremock DRS, no Ferrum, no Docker) | Local; CI workflow **CI** (file `.github/workflows/conformance.yml`) on **push** `main`/`master`, **pull_request** to those branches, **workflow_dispatch**. Job `build-and-test` skips `dependabot[bot]`. |
| SPDX on first-party `.rs` | `make prove` → `scripts/spdx-rs.py`; also workflow **SPDX** (`spdx.yml`) | Offline | `make prove`; SPDX workflow: **push** `main`/`master`, **PR**, **workflow_dispatch** |
| Release CLI build | `make prove` → `cargo build --release -p helixtest-cli` | Offline compile | Same as `make prove` (CI prove job) |
| rustfmt + clippy | **Not** in `Makefile`. CI job `build-and-test` and `scripts/hooks/ci-check.sh` | Offline | CI **CI** workflow (PR / push / dispatch); local if you run the hook. `.pre-commit-config.yaml` maps `ci-parity` → `ci-check.sh` (fmt + clippy + `make test`, **not** SPDX / release binary) |
| MSRV compile | `conformance.yml` job `msrv`: `cargo check --locked --workspace --all-targets` on **1.88.0** (includes live-stack crates as **compile**, not `cargo test`) | Offline compile; does **not** execute live HTTP | Same **CI** workflow; skipped for `dependabot[bot]` |
| ARM64 offline prove | `conformance.yml` job `build-arm64`: `make prove` on `ubuntu-24.04-arm` | Offline | **workflow_dispatch**, or **push** to `main`/`master` only — **not** on pull_request |
| Live `framework::run_all` subsets | `api-tests` (WES+DRS+TRS), `workflow-tests` (WES), `auth-tests` (Auth), `e2e-tests` (E2E) | **Live target** (env/profile URLs). Excluded from `make prove` | Local only unless you `cargo test -p …` with a stack. **Not** in default CI |
| HelixTest CLI vs published Ferrum GHCR (auth **off**, demo) | `.github/workflows/live-ferrum-ghcr.yml` (`Live Ferrum GHCR`) | **Live** Docker `ghcr.io/synapticfour/ferrum:edge` (overridable). Default `--only beacon` on schedule (edge has no WES; comments say `--all` hit TRS `:8083` and connection-refused) | **cron** `17 4 * * 1`; **workflow_dispatch**. **Does not run on pull requests** |
| HS256 auth-on vs published Ferrum GHCR | `.github/workflows/live-ferrum-ghcr-auth.yml` (`Live Ferrum GHCR auth-on`) | **Live** Docker, `require_auth=true`. Python/curl HS256 proof + `helixtest --only auth --fail-level 4` (`HELIXTEST_AUTH_SURFACE=service-info`). **Not** Passports / ga4gh-infra | **cron** `27 4 * * 1`; **workflow_dispatch**. **Not** on PR |
| Gitleaks | `secret-scan.yml` | Offline (repo scan) | **pull_request**; **push** `main`/`master`. No `workflow_dispatch` in this file |
| CodeQL (Rust) | `codeql.yml` | Autobuild + analyze. SARIF upload **never** unless `vars.CODEQL_UPLOAD_SARIF` or `.github/ci-config.json` says otherwise | **cron** `22 3 * * 1`; **workflow_dispatch**. **Not** on PR/push |
| Dependency Review | `dependency-review.yml` | GitHub dependency graph. `continue-on-error: true` | **pull_request** only |
| Release assets (`helixtest` binaries) | `release-binaries.yml` | Build, not a conformance suite | **push** tags `v*`; **workflow_dispatch** |

`rust-toolchain.toml` pins **1.91.1**. Workspace `rust-version` is **1.88**. CI prove job installs `dtolnay/rust-toolchain@stable`. Whether `stable` always matches 1.91.1: **UNKLAR — bitte prüfen**.

---

## 2. Bekannte Lücken

Copied from [`helixtest/docs/known-limitations.md`](helixtest/docs/known-limitations.md) (wording unchanged):

Intentional remaining constraints after closing the 14 Aug 2026 follow-up gaps.

### Serial HTTP

WES/TES cases stay **serial** so the target is not overloaded. Independent services still run one after another. A down host fails faster than the old 5×60s retry (5s connect, two GET attempts). Parallel service checks are out of scope.

### Live-stack cargo tests

`api-tests`, `auth-tests`, `e2e-tests`, and `workflow-tests` now call `framework::run_all` (same checks as `helixtest --all`). They still need a running stack and stay **excluded** from default CI / `scripts/hooks/ci-check.sh`. In-process age checks live in `crypt4gh-tests` and run in CI.

### Africa / Infra modes

`--only africa` / `--only infra` in generic `--all` are recorded as skipped with “use `--mode ferrum-africa` or `--mode ferrum+infra`”. Those suites are not mixed into the default ladder.

### jsonschema `'static` leak

jsonschema 0.17 compiles from `&'static Value`. Each official schema is `Box::leak`’d **once** when first compiled (`OnceCell`). A leak-free API needs jsonschema ≥0.26. Left on 0.17 to avoid pulling reqwest 0.12 alongside the workspace’s reqwest 0.11. The old per-call leak in `validate_json_against` is gone (that helper was removed).

### `once_cell`

`ga4gh_schemas` uses `once_cell::OnceCell::get_or_try_init`. `std::sync::OnceLock::get_or_try_init` is still unstable (`once_cell_try`).

*HelixTest by Synaptic Four — Apache-2.0.*

### Zusätzlich im Code (kein `TODO`/`FIXME`/`XXX`/`HACK`/`unimplemented!` in `*.rs` / `*.toml` / workflows — `rg` leer)

- **`#[ignore]`:** `helixtest-common` `http.rs` `robustness_timeout_fails_fast` is ignored (`slow when client timeout does not fire`). The ignore message says `cargo test -p common -- --ignored`. The Cargo **package** name is `helixtest-common` (lib name `common`). **UNKLAR — bitte prüfen** whether `-p common` works in this workspace; `-p helixtest-common` matches `Cargo.toml`.
- **`framework/src/e2e.rs`:** drives WES to terminal `COMPLETE`; **does not poll TES**. The module comment says full TRS→…→TES coupling “lives in the `e2e-tests` crate only when the mock stack defines that contract”. `e2e-tests` only calls `run_all(..., E2e)` — no extra TES poll in that crate. **UNKLAR — bitte prüfen** what “mock stack contract” that sentence refers to.
- **Auth skips (runtime, not missing tests):** HMAC JWT fixture skipped if `HELIXTEST_SHARED_SECRET` unset (not Passports). `HELIXTEST_SKIP_AUTH=true` in `Mode::Ferrum` replaces the auth suite with one skip. Missing Bearer on DRS `/service-info` skipped when `HELIXTEST_AUTH_SURFACE=service-info` (public metadata). Passports are `--mode ferrum+infra` only.
- **DRS:** checksum check skipped unless profile `[features] strict_drs_checksums = true`.
- **WES:** scatter/gather skipped unless `supports_scatter_gather`.
- **htsget:** whole suite skipped if URL unresolved; dataset-gated L4 skipped without `HELIXTEST_HTSGET_DATASET_OBJECT_ID`; CRAM-on-BAM assertion skipped if object already CRAM; generic split-port mocks skip htsget (comment in `htsget.rs`).
- **Crypt4GH HTTP** (`crypt4gh_ferrum_http.rs`): skipped unless `HELIXTEST_FEATURE_CRYPT4GH_REWRAP` / `HELIXTEST_FEATURE_CRYPT4GH_PLAIN`. `crypt4gh-tests` crate is **age** files, not Ferrum HTTP Crypt4GH.
- **Beacon:** known/negative variant checks skipped unless `supports_beacon_v2`.
- **`africa.rs`:** federation / outbreak paths skip when peer URL down or auth required (env-gated).
- **`helixtest/docker/docker-compose.yml`:** `ghcr.io/example/mock-*` are **not pullable**. 2026-09-04, Docker 29.7.2: `docker manifest inspect` → `manifest unknown` for wes/tes/drs/trs/beacon/oidc. Do not use `--start-ferrum` / `--start-compose` with that default file. Re-check: `docker manifest inspect ghcr.io/example/mock-wes:latest`.
- **Live GHCR workflows** are demo/eval-shaped (auth off, or HS256 — not Passport). They are not PR gates and not a hospital proof.
- **Dependency Review** is non-fatal (`continue-on-error`).
- **Green CI is not certification.** Skip is not pass.

---

## 3. Lokal testen, bevor du einen PR öffnest

Needs Rust (see `rust-toolchain.toml` / MSRV 1.88). No Ferrum required for the offline path.

```bash
# Closest to the PR prove job (offline tests + SPDX + release CLI)
make prove

# Same cargo test excludes as CI, without SPDX / release binary
make test

# CI parity used by pre-commit: fmt + clippy -D warnings + make test
./scripts/hooks/ci-check.sh
```

Einzelne **offline** crates (package names):

```bash
cargo test -p helixtest-common
cargo test -p helixtest-framework
cargo test -p crypt4gh-tests
cargo test -p helixtest-cli
```

Ignored HTTP timeout test (see §2):

```bash
cargo test -p helixtest-common -- --ignored
```

**Live-stack crates** (need URLs / a running target; will fail or skip-heavy without one):

```bash
cargo test -p api-tests
cargo test -p auth-tests
cargo test -p e2e-tests
cargo test -p workflow-tests
```

CLI against a stack you started (not started by HelixTest): see [docs/PROVE.md](docs/PROVE.md). Example after `cd ../Ferrum && make up`:

```bash
helixtest --all --mode ferrum
```

Behavior changes should add or update tests in the crate that owns the behavior, or document why a test is not feasible (live-only surface, env-gated skip).
