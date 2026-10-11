# CONTINUITY — HelixTest

Survivability notes. **True on `origin/main` today.** Not legal advice. Not GA4GH certification.

**Audited revision:** `origin/main` @ `b7b24a4238ca17b4e242dff7085297fc0b69f408`. Latest tag on origin: `v0.1.3` (commit `1832c043e1679ec283cb2113510ee33684317cce`, differs from this main tip). Package version on main (`helixtest-cli`): `0.1.0`.

## What this project is

Rust GA4GH conformance / integration suite. CLI binary **`helixtest`**. License: **Apache-2.0**. Does **not** start servers — point it at a running target (Ferrum, generic DRS, or Ferrum+infra). The VERIFY product wrapper is [Helix](https://github.com/SynapticFour/Helix). HELIOS is unrelated (pipeline evidence).

## Build from a clean machine (from a given tag)

### Prerequisites

- Git, `make` (Debian/Ubuntu: install the `make` package — missing in a bare `rust:*` container)
- rustup — toolchain from `rust-toolchain.toml` (**1.91.1** on audited main); README also states MSRV **1.88+**
- Network to crates.io for first build
- **libsodium** (dev headers) + `pkg-config` — Debian/Ubuntu: `libsodium-dev pkg-config`; macOS: `brew install libsodium` (also in `helixtest/README.md`)
- Optional live targets: a running Ferrum / infra stack (separate repos)
- In containers: put `cargo` on `PATH` (e.g. `/usr/local/cargo/bin`)

### Source

```bash
git clone https://github.com/SynapticFour/HelixTest.git
cd HelixTest
git checkout v0.1.3    # or main / another v* tag
```

### Prove (CI-parity offline)

```bash
make prove
# = offline cargo tests (excludes live-stack crates) + cargo build --release -p helixtest-cli
# Binary: target/release/helixtest
```

Install alias: `make install` → release CLI build.

Binary install without full clone: see `docs/INSTALL.md`.

## How to run tests

| Command | Meaning |
|---------|---------|
| `make test` / `make prove` | Offline unit tests + release CLI |
| `helixtest --all --mode ferrum` | Live against Ferrum demo you started |
| `helixtest --all --mode generic --only drs …` | Live against any DRS |
| `helixtest --all --mode ferrum+infra --profile ferrum-infra` | Co-deploy broker + Passport-on-DRS |

Live stacks are **not** started by HelixTest. See `docs/PROVE.md`, `docs/ECOSYSTEM.md`.

## How to cut a release

Per `RELEASING.md`:

1. CI green on `main`.
2. Update `CHANGELOG.md`.
3. Annotated tag `vX.Y.Z` → `git push origin vX.Y.Z`.
4. Verify GitHub release artifacts/notes.

Consumers (Helix `VERSIONS.lock`, `helixtest-action`) pin a **SHA** and/or tag — after tagging, update those pins in the same release train if behaviour changes.

## Continuity

HelixTest is maintained by a single steward (Synaptic Four). To reduce dependence on any one person, the build is documented from a clean machine in this file, the release steps are written down, and the source can be inspected and built without access to Synaptic Four systems (clean-room check dated 2026-10; requires libsodium development headers and pkg-config). Operational details such as accounts, publication rights and escrow arrangements are kept in a non-public continuity plan. If you depend on HelixTest in production, ask for it.

## Minimal operator runbook

| Action | Command |
|--------|---------|
| Install | Clone tag → rustup → install libsodium → `make prove` |
| Start | N/A (CLI only) — start **target** elsewhere |
| Run checks | `./target/release/helixtest --help` then mode/profile as above |
| Stop / back up / restore | N/A for the CLI; retain `CHANGELOG` + tag + any JSON reports you generate |
| Upgrade | Checkout newer tag; rebuild; update consumer pins |
