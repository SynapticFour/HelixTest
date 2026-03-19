# Testing Ferrum with HelixTest

This guide describes how to run the HelixTest conformance suite against **Ferrum**, a Rust-based GA4GH platform.

## Quick start

1. **Use the Ferrum profile** (endpoints + feature flags):
   ```bash
   export HELIXTEST_PROFILE=ferrum
   ```

2. **Optional: start Ferrum with Docker** (from the suite or project root):
   ```bash
   cd docker   # or your Ferrum docker-compose location
   docker compose up -d
   ```
   Or let the CLI start it:
   ```bash
   helixtest --all --mode ferrum --start-ferrum
   ```

3. **Run the suite**:
   ```bash
   cargo run --bin helixtest -- --all --mode ferrum
   ```
   Or, with profile set, the same run will use `profiles/ferrum.toml` for both endpoints and features.

## Profile: `profiles/ferrum.toml`

The Ferrum profile sets:

- **[services]** – **single-gateway** layout matching [Ferrum’s HelixTest integration](https://github.com/SynapticFour/Ferrum/blob/main/docs/HELIXTEST-INTEGRATION.md): `http://localhost:8080/ga4gh/{wes|drs|tes|trs|…}/v…` and `passports/v1` on the same origin.
- **[features]** – `supports_scatter_gather`, `supports_beacon_v2`, `strict_drs_checksums` (all true for Ferrum).

Override endpoints via `HELIXTEST_CONFIG` or environment variables (`WES_URL`, etc.) if your deployment uses different hosts or paths.

### htsget 1.3.0 (Ferrum `ferrum-htsget`)

Ferrum mounts GA4GH **htsget 1.3.0** at **`/ga4gh/htsget/v1`** (see [Ferrum](https://github.com/SynapticFour/Ferrum)). HelixTest resolves the htsget base automatically when:

- **`WES_URL` / `DRS_URL` / …** use a `/ga4gh/…` path (gateway prefix stripped → `{origin}/ga4gh/htsget/v1`), or
- **`--mode ferrum`** and a service URL is a bare `http://host:port` (treated as unified gateway), or
- **`GATEWAY_BASE`**, **`HTSGET_URL`**, or **`[services] htsget`** is set explicitly.

With **`HELIXTEST_PROFILE=ferrum`** (or matching env URLs), **no extra htsget env vars are required** for the default suite.

**Conformance checks** (aligned with Ferrum handlers):

| Area | What is tested |
|------|----------------|
| Service-info | `GET …/reads/service-info` and `…/variants/service-info` — `type` (`org.ga4gh` / `htsget` / **1.3.0**), `htsget.datatype`, `formats` (reads: **BAM** + CRAM; variants: **VCF**/BCF), `fieldsParameterEffective` / `tagsParametersEffective` **false**. |
| Tickets | `GET`/`POST` reads & variants; `Content-Type` `application/vnd.ga4gh.htsget…` or JSON; ticket `urls[0].url` path **…/ga4gh/drs/v1/objects/{id}/stream**. |
| Errors | Variants URL with a **reads-only** id → **404** `NotFound`; `POST` with **query string** → **400** `InvalidInput`; `GET ?format=CRAM` on a **BAM** object → **400** `UnsupportedFormat`; `GET ?class=header` → **400** `InvalidInput`. |

**Object IDs** (Ferrum demo / HelixTest DRS):

- **Reads (BAM):** `HTSGET_READS_OBJECT_ID` or legacy `HTSGET_READS_ID` — default **`test-object-1`**.
- **Variants (VCF):** `HTSGET_VARIANTS_OBJECT_ID` — default **`demo-sample-vcf`** (E2E seed in Ferrum docs).

**Dataset auth (optional):** When an object has `dataset_id` and `FERRUM_AUTH__REQUIRE_AUTH=true`, set **`HELIXTEST_HTSGET_DATASET_OBJECT_ID`** — expect **403** `PermissionDenied` without `Authorization`. For the success path, set **`HELIXTEST_HTSGET_DATASET_BEARER`** to a GA4GH Passport / token with **ControlledAccessGrants** for that dataset (plain HelixTest JWT scope is not enough). If only the object id is set, the test passes after asserting 403 and notes that Bearer was not supplied.

**Generic / split-port mocks:** With `profiles/generic.toml` (no `/ga4gh/` paths) and **`--mode generic`**, htsget is **skipped** unless you set `HTSGET_URL` or `GATEWAY_BASE` explicitly — so split mock stacks are not broken by htsget.

### Crypt4GH: rewrap vs decrypt_plain

Optional **Level 3** checks (see main [README](../README.md#crypt4gh-ferrum-rewrap-vs-decrypt-plain-optional)):

- **`HELIXTEST_FEATURE_CRYPT4GH_REWRAP=1`** – DRS download with `X-Crypt4GH-Public-Key`, local Crypt4GH decrypt; needs `CRYPT4GH_CLIENT_SECRET_KEY_PATH`.
- **`HELIXTEST_FEATURE_CRYPT4GH_PLAIN=1`** – compares plaintext SHA256 from a dedicated plain URL vs rewrap-decrypted bytes; requires rewrap enabled and **`C4_PLAIN_DOWNLOAD_URL`** or **`C4_PLAIN_URL_BASE`** (+ optional **`C4_PLAIN_URL_PATH`**).

Ferrum must serve **decrypt_plain** on the URL you configure (server-side `stream_decrypt` while data stays Crypt4GH at rest).

## Mode: generic vs ferrum

- **`--mode ferrum`** – Loads features from `profiles/ferrum.toml` (if no `HELIXTEST_PROFILE` is set) and skips Ferrum auto-detection.
- **`--mode generic`** (default) – If WES `/service-info` reports a name containing "Ferrum", the framework switches to Ferrum mode automatically so the right feature flags are used.

Using `HELIXTEST_PROFILE=ferrum` is equivalent for feature/endpoint loading and works with either mode.

## What is exercised

- WES lifecycle, TRS-based workflows, DRS access and checksums, **htsget 1.3.0** (service-info, GET/POST tickets, DRS stream URLs, error codes) on the gateway profile.
- TES task lifecycle and output checksums.
- Beacon v2 (if enabled by profile).
- Auth (Level 4) and Crypt4GH (Level 5) when endpoints are configured.
- E2E pipeline: TRS → DRS → WES → TES → DRS → Beacon.

See the main [README](../README.md) and [architecture](architecture.md) for full scope.

---

*HelixTest by Synaptic Four — built for the open science community. © 2025 Synaptic Four · Apache-2.0.*
