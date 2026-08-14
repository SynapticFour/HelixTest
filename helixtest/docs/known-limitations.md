# Known limitations

Items left after the 14 Aug 2026 scoring/HTTP/workspace hardening. None of these restore skip-as-pass or POST retries.

## Auth is an HMAC fixture, not Passports

Default Level 4 Auth mints **HS256 JWTs** (`common::auth::build_jwt`) against DRS, with `HELIXTEST_SHARED_SECRET` or a built-in **`test-secret`** (warns when unset). That secret is for demo stacks only. **GA4GH Passports / OIDC** are `--mode ferrum+infra` (`infra.rs`), not the default ladder.

## Local “Crypt4GH” Level 5 is age

`ServiceKind::Crypt4gh` and the `crypt4gh-tests` crate still use that name. Local encrypt/decrypt is **age**. Real Crypt4GH (libsodium `crypt4gh` crate) runs only when `HELIXTEST_FEATURE_CRYPT4GH_*` is set. A generic target can therefore achieve Crypt4GH Level 5 without speaking Crypt4GH. Optional HTTP rewrap still buffers the full object in memory (`bytes()`), then decrypts — fine for demo objects, not BAM-sized payloads.

## Achieved level can skip empty rungs

Highest N such that every **executed** test at N passed. Empty or skip-only levels do not block a higher N. Crypt4GH can report Level 5 with no L0–L4 tests. See [scoring.md](scoring.md).

## Two runners

Live-stack crates (`api-tests`, `auth-tests`, `e2e-tests`, `workflow-tests`) are excluded from default CI and from `scripts/hooks/ci-check.sh`. Run them against a running target with `cargo test -p …`.

## Schema stacks

Official OpenAPI validation lives in `common::ga4gh_schemas` (preferred). `validate_json_against<T>` (schemars) remains for ad-hoc types. jsonschema 0.17 requires `'static` schemas, so each distinct schema is `Box::leak`’d **once** (cached). Upgrading jsonschema would remove the leak.

## MSRV

`package.rust-version = "1.75"` is set. CI uses **stable**, not `cargo +1.75 check`. `once_cell` in `ga4gh_schemas` could be `std::sync::OnceLock`.

## Lockfile

`**/Cargo.lock` is gitignored. CI therefore resolves workspace deps on each run rather than a pinned lockfile. Pinning would be a separate change (typical for a binary CLI).

## Africa / Infra modes

Those suites run only via `--mode ferrum-africa` / `ferrum+infra`, not `--only africa`. Listing them in a subset profile’s `enabled_services` does not execute them.

## Serial HTTP

WES/TES cases stay serial so the target is not overloaded. A down host fails faster than before (5s connect, two GET retries) but the suite is still one service after another.

---

*HelixTest by Synaptic Four — Apache-2.0.*
