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

- **[services]** – default local URLs (e.g. WES on 8080, TES on 8081, …).
- **[features]** – `supports_scatter_gather`, `supports_beacon_v2`, `strict_drs_checksums` (all true for Ferrum).

Override endpoints via `HELIXTEST_CONFIG` or environment variables (`WES_URL`, etc.) if your deployment uses different ports or hosts.

## Mode: generic vs ferrum

- **`--mode ferrum`** – Loads features from `profiles/ferrum.toml` (if no `HELIXTEST_PROFILE` is set) and skips Ferrum auto-detection.
- **`--mode generic`** (default) – If WES `/service-info` reports a name containing "Ferrum", the framework switches to Ferrum mode automatically so the right feature flags are used.

Using `HELIXTEST_PROFILE=ferrum` is equivalent for feature/endpoint loading and works with either mode.

## What is exercised

- WES lifecycle, TRS-based workflows, DRS access and checksums.
- TES task lifecycle and output checksums.
- Beacon v2 (if enabled by profile).
- Auth (Level 4) and Crypt4GH (Level 5) when endpoints are configured.
- E2E pipeline: TRS → DRS → WES → TES → DRS → Beacon.

See the main [README](../README.md) and [architecture](architecture.md) for full scope.

---

*HelixTest by Synaptic Four — built for the open science community. © 2025 Synaptic Four · Apache-2.0.*
