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
