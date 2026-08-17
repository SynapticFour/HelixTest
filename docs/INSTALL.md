# Install HelixTest without cloning the suite

HelixTest is a **free Apache-2.0 ambassador** CLI (`helixtest`). It is not a server.

**MSRV is Rust 1.88.0** (encoded in the workspace). This repo’s `rust-toolchain.toml` is **1.91.1** (same channel as Ferrum / Lab-Kit / ga4gh-infra / Solum). CI keeps an explicit 1.88 MSRV job so third parties can still build on the older compiler.

## One binary

Prefer a GitHub Release asset. Fallback: Cargo (Rust 1.88+; libsodium for the workspace):

```bash
# Linux x86_64 (sha256 next to the asset). 404 → cargo install below.
curl -fsSL -o helixtest \
  https://github.com/SynapticFour/HelixTest/releases/download/v0.1.1/helixtest-x86_64-unknown-linux-gnu
chmod +x helixtest

# Fallback (needs Rust + libsodium-dev / brew libsodium):
cargo install --git https://github.com/SynapticFour/HelixTest.git \
  --tag v0.1.1 --locked --bin helixtest
```

Attach missing assets: Actions → **Release binaries** → `workflow_dispatch` with tag `v0.1.1`. `cargo binstall` is not published.

## One URL, one report

```bash
helixtest --all --mode ferrum --report json --fail-level 2
# or a single surface:
helixtest --all --mode ferrum --only beacon --report json
```

Point the CLI at a running target (env vars / profile — see [PROVE.md](PROVE.md)). Results are **not** official GA4GH certification.

## Public proof against published Ferrum

Opt-in (does not run on every PR): GitHub Actions workflow **Live Ferrum GHCR** (`workflow_dispatch`, plus a weekly schedule) pulls `ghcr.io/synapticfour/ferrum:edge` (HEAD publish), starts it **auth-off / SQLite demo-mode**, and runs HelixTest. Tag `v0.3.1-edge` is schema-behind Beacon boolean `meta` — do not use it as the default. Not a hospital auth-on proof, and not the Demo overlay path.

**Published image, auth on (HS256):** workflow **Live Ferrum GHCR auth-on** starts the **same** `:edge` default with `FERRUM_AUTH__REQUIRE_AUTH=true` and a minted secret. It proves garbage Bearer → 401 and a valid HS256 JWT → 200 on DRS `service-info`. That matches Ferrum `make eval`. It is **not** ga4gh-infra Passports (Ferrum workflow `helixtest-ferrum-infra.yml`).

**Co-deploy (Ferrum + ga4gh-infra):** pin Ferrum **v0.3.1** and **ga4gh-infra-v0.2.3** (same as Ferrum `VERSIONS.lock`). Local: Ferrum `make up-pilot-local` then `helixtest --all --mode ferrum+infra --profile ferrum-infra-pilot`. Hosted proof: Ferrum workflow **ferrum+infra (HelixTest)** (`helixtest-ferrum-infra.yml`, schedule + dispatch). Not GA4GH certification.

**GitHub Action for third parties:** [synapticfour/helixtest-action](https://github.com/SynapticFour/helixtest-action) wraps the v0.1.1 release binary. Schema source of truth remains the published GA4GH OpenAPI (vendored in this repo).
