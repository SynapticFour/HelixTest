## HelixTest — GA4GH Conformance Suite

This repository is **HelixTest**, a Rust-based conformance and integration test suite for GA4GH-style platforms. The project and CLI are named HelixTest; the binary is `helixtest`.

Scope boundary: HelixTest remains **GA4GH-focused**. MII/KDS checks are intentionally handled in Ferrum/Ferrum-Lab-Kit integration layers, not in this suite.

**SynapticFour GA4GH stack:** see **[docs/ECOSYSTEM.md](docs/ECOSYSTEM.md)** for how HelixTest relates to Ferrum, ga4gh-infra, Lab Kit, and Demo.

Implemented test areas:

- **API contract tests** for WES, TES, DRS, TRS, Beacon v2
- **Workflow execution tests** for CWL, WDL, Nextflow via WES
- **Cross-service E2E tests** spanning TRS → DRS → WES → TES → Beacon
- **Authorization tests** for GA4GH Passports / OIDC-style flows
- **Co-deploy tests** via `--mode ferrum+infra` and profile `ferrum-infra` (broker, registry, Passport-on-DRS)
- **Cryptographic tests** for Crypt4GH-style encryption (backed by `age` as a pluggable engine)

See `helixtest/README.md` for full details and usage. For **Ferrum**: HelixTest is the conformance runner; **noop TES** is the usual CI default — **Docker TES** for demos is described in `helixtest/docs/adr/0001-ferrum-tes-ci-vs-docker-stack-and-db-init.md`.

> **Legal notice:** This repository documents technical capabilities and operating guidance. It is not legal advice and does not by itself provide regulatory certification or compliance guarantees. Compliance outcomes depend on operator configuration, contracts, and organisational controls.

**Disclaimer:** This software is provided as is, without warranty. Test results do not constitute official GA4GH certification. See [LICENSE](LICENSE) for full terms.

---

Synaptic Four · Contact: [contact@synapticfour.com](mailto:contact@synapticfour.com) · [synapticfour.com](https://synapticfour.com)

