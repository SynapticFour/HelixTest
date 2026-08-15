# Install HelixTest without cloning the suite

HelixTest is a **free Apache-2.0 ambassador** CLI (`helixtest`). It is not a server.

**MSRV is Rust 1.88.0** (encoded in the workspace). Ferrum/Lab-Kit/ga4gh-infra currently use 1.91.1; HelixTest stays on 1.88 so third parties can build on the older stable compiler. CI has an explicit 1.88 MSRV job.

## One binary

Until GitHub Release assets exist for a tag, the supported no-clone install is Cargo (Rust 1.88+):

```bash
# libsodium is required (crypt4gh tests crate in the workspace)
# macOS: brew install libsodium
# Debian/Ubuntu: sudo apt-get install -y libsodium-dev pkg-config

cargo install --git https://github.com/SynapticFour/HelixTest.git \
  --tag v0.1.1 --locked --bin helixtest
```

Pushing a `v*` tag runs `.github/workflows/release-binaries.yml` and attaches
`helixtest-*` binaries to the GitHub Release. `cargo binstall` is not published yet.

## One URL, one report

```bash
helixtest --all --mode ferrum --report json --fail-level 2
# or a single surface:
helixtest --all --mode ferrum --only beacon --report json
```

Point the CLI at a running target (env vars / profile — see [PROVE.md](PROVE.md)). Results are **not** official GA4GH certification.

## Public proof against published Ferrum

Opt-in (does not run on every PR): GitHub Actions workflow **Live Ferrum GHCR** (`workflow_dispatch`) pulls `ghcr.io/synapticfour/ferrum:v0.3.0-edge`, starts it **auth-off / SQLite demo-mode**, and runs HelixTest. That is the public claim that a tagged Ferrum image answers Beacon/DRS HTTP — not a hospital auth-on proof, and not the Demo overlay path.
