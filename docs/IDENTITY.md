# Who HelixTest is for

HelixTest (`helixtest`) is a **free Apache-2.0 ambassador**: a conformance CLI you point at a running GA4GH target. It is **not** a product SKU and **not** a server.

## Audience

Anyone implementing or buying a GA4GH API — including Ferrum, including competitors.

**Not for:** deploying Beacon/DRS/WES (that is Ferrum), issuing Passports (ga4gh-infra).

## Standalone

```bash
# No-clone install (Rust 1.88+): see docs/INSTALL.md
cargo install --git https://github.com/SynapticFour/HelixTest.git \
  --tag v0.1.1 --locked --bin helixtest

git clone https://github.com/SynapticFour/HelixTest.git && cd HelixTest
make prove
# Against a stack you started:
helixtest --all --mode ferrum
helixtest --all --mode ferrum+infra --profile ferrum-infra
helixtest --all --mode ferrum+infra --profile ferrum-infra-pilot   # Ferrum make up-pilot-local (:8080)
```

Third-party GitHub Actions: [synapticfour/helixtest-action](https://github.com/SynapticFour/helixtest-action) (same `v0.1.1` binaries). The action does not start Ferrum.

Results are **not** official GA4GH certification. Product pin is git tag **v0.1.1** (`a8aabf30…`) — same SHA as Ferrum `VERSIONS.lock`. The CLI crate `version` in `Cargo.toml` may stay `0.1.0`; operators pin the **tag**, not the crate number.

**Ferrum HTTP:** HelixTest validates against the **published GA4GH OpenAPI** for each standard (vendored under `helixtest/schemas/ga4gh/`). Ferrum’s [utoipa dump](https://github.com/SynapticFour/Ferrum/blob/main/docs/openapi/ferrum.openapi.json) is an implementation map (gateway paths, Ferrum-only additions), not a second spec.
