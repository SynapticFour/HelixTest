# Subset Profiles

Subset profiles let HelixTest validate only the GA4GH modules a platform actually implements.

## bioresearch-assistant profile

Profile file: `profiles/bioresearch-assistant.toml`

- Enabled services: `wes`, `drs`, `auth`
- Disabled services: `tes`, `trs`, `beacon`, `htsget`, `crypt4gh`, `e2e`
- Auth mode: `token-protected-endpoints`

## Required environment variables

- `WES_URL` (optional override for profile default)
- `DRS_URL` (optional override for profile default)
- `TEST_BEARER` (optional but recommended; valid token for `2xx` auth checks)

Optional:

- `AUTH_URL` (not required for token-only mode, but accepted)
- `HELIXTEST_CONFIG` (if you want a custom config instead of profile defaults)

## Local run

```bash
cargo run --bin helixtest -- \
  --all \
  --profile bioresearch-assistant \
  --mode generic \
  --report table \
  --fail-level 1
```

Optional explicit module selection (equivalent with this profile):

```bash
cargo run --bin helixtest -- \
  --all \
  --profile bioresearch-assistant \
  --only wes --only drs --only auth
```

## CI run

```bash
export WES_URL="https://your-platform.example/ga4gh/wes/v1"
export DRS_URL="https://your-platform.example/ga4gh/drs/v1"
export TEST_BEARER="${BIORESEARCH_ASSISTANT_TEST_BEARER}"

cargo run --bin helixtest -- \
  --all \
  --profile bioresearch-assistant \
  --report json \
  --fail-level 1 > helix-report.json
```

### GitHub Actions snippet

```yaml
- name: HelixTest subset conformance
  env:
    WES_URL: ${{ secrets.WES_URL }}
    DRS_URL: ${{ secrets.DRS_URL }}
    TEST_BEARER: ${{ secrets.TEST_BEARER }}
  run: |
    cargo run --bin helixtest -- \
      --all \
      --profile bioresearch-assistant \
      --report json \
      --fail-level 1 > helix-report.json
```

## Notes

- Exit code is non-zero only if enabled modules fail (or `--fail-level` is not met for enabled modules).
- Disabled modules are reported as skipped and do not fail the run.

## Ferrum: narrower PR-style path (still conformance-only)

Full `--all` against `HELIXTEST_PROFILE=ferrum` exercises WES, TES, DRS, TRS, Beacon, htsget, Auth, Crypt4GH, and E2E — useful for release validation, heavier on CI time and dependencies.

For a **smaller** run that still checks core GA4GH surfaces **without** turning HelixTest into a benchmark (no time budgets as gates), combine profile or env URLs with `--only`:

```bash
export HELIXTEST_PROFILE=ferrum
# optional: WES_URL / DRS_URL overrides

cargo run --bin helixtest -- \
  --all \
  --mode ferrum \
  --only wes \
  --only drs \
  --only crypt4gh \
  --report json \
  --fail-level 1
```

- **DRS** stays on contract + byte integrity; **Crypt4GH** keeps local Level 5 checks; optional Ferrum HTTP Crypt4GH remains env-gated (`HELIXTEST_FEATURE_CRYPT4GH_*`).
- Add `--only htsget` or `--only tes` when those subsystems are in scope for the PR.

See **[conformance-philosophy.md](conformance-philosophy.md)** for why this is not a performance profile.

## Rough resource hints (operators)

Order-of-magnitude only — your stack and data sizes dominate.

| Profile / run | RAM | Disk |
|---------------|-----|------|
| **bioresearch-assistant** (WES + DRS + token auth) | ~2–4 GiB free for JVM/Rust services + client | ~2–5 GiB for images/logs |
| **ferrum** full `--all` (gateway + workflows + optional Crypt4GH) | ~8–16 GiB typical for Cromwell/WES + TES + DRS on one host | ~20+ GiB if pulling many images and workflow outputs |

Tight CI runners should prefer **subset** runs and mocks; use full Ferrum profile on larger agents or nightly jobs.
