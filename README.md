# HelixTest — GA4GH Conformance Suite

HelixTest is a Rust-based conformance and integration test suite for GA4GH-style platforms. The CLI binary is **`helixtest`**.

> **Not a deployable stack:** HelixTest does not run servers. Start a target first (Ferrum, ga4gh-infra, or both), then run HelixTest against it. See **[docs/ECOSYSTEM.md](docs/ECOSYSTEM.md)** for unified lifecycle commands on sibling repos.

Scope boundary: HelixTest remains **GA4GH-focused**. MII/KDS checks live in Ferrum / Ferrum-Lab-Kit integration layers.

### Quick start

**Prerequisites:** [Rust](https://rustup.rs) 1.75+

```bash
git clone https://github.com/SynapticFour/HelixTest.git && cd HelixTest
cargo build --release -p helixtest-cli
export PATH="$PWD/target/release:$PATH"
```

**Run against a local Ferrum demo** (start Ferrum first — see [Ferrum README](https://github.com/SynapticFour/Ferrum)):

```bash
# In Ferrum repo: make up
helixtest --all --mode ferrum
```

**Co-deploy (Ferrum + ga4gh-infra):**

```bash
# In Ferrum-GA4GH-Demo: make up-with-infra
helixtest --all --mode ferrum+infra --profile ferrum-infra
```

| Command | Purpose |
|---------|---------|
| `helixtest --all --mode ferrum` | Full suite against Ferrum demo stack |
| `helixtest --all --mode ferrum+infra --profile ferrum-infra` | Co-deploy broker + Passport-on-DRS |
| `helixtest --service wes --mode ferrum` | Single service |
| `helixtest --help` | All flags, report formats, fail levels |

Full usage, architecture, and CI integration: **[helixtest/README.md](helixtest/README.md)**.

### Test areas

- **API contract tests** for WES, TES, DRS, TRS, Beacon v2
- **Workflow execution tests** for CWL, WDL, Nextflow via WES
- **Cross-service E2E tests** spanning TRS → DRS → WES → TES → Beacon
- **Authorization tests** for GA4GH Passports / OIDC-style flows
- **Co-deploy tests** via `--mode ferrum+infra` and profile `ferrum-infra`
- **Cryptographic tests** for Crypt4GH-style encryption

For Ferrum-specific guidance: [helixtest/docs/ferrum.md](helixtest/docs/ferrum.md). **CI vs full stack:** noop TES in CI; Docker TES documented in [helixtest/docs/adr/0001-ferrum-tes-ci-vs-docker-stack-and-db-init.md](helixtest/docs/adr/0001-ferrum-tes-ci-vs-docker-stack-and-db-init.md).

> **Legal notice:** This repository documents technical capabilities and operating guidance. It is not legal advice and does not by itself provide regulatory certification or compliance guarantees.

**Disclaimer:** Test results do not constitute official GA4GH certification. See [LICENSE](LICENSE).

---

Synaptic Four · [contact@synapticfour.com](mailto:contact@synapticfour.com) · [synapticfour.com](https://synapticfour.com)
