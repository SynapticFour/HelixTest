## HelixTest — GA4GH Conformance Suite

This repository is **HelixTest**, a Rust-based conformance and integration test suite for GA4GH-style platforms. The project and CLI are named HelixTest; the binary is `helixtest`.

Implemented test areas:

- **API contract tests** for WES, TES, DRS, TRS, Beacon v2
- **Workflow execution tests** for CWL, WDL, Nextflow via WES
- **Cross-service E2E tests** spanning TRS → DRS → WES → TES → Beacon
- **Authorization tests** for GA4GH Passports / OIDC-style flows
- **Cryptographic tests** for Crypt4GH-style encryption (backed by `age` as a pluggable engine)

See `helixtest/README.md` for full details and usage.

**Disclaimer:** This software is provided as is, without warranty. Test results do not constitute official GA4GH certification. See [LICENSE](LICENSE) for full terms.

**Renaming the GitHub repo to HelixTest:** In GitHub, go to **Settings → General → Repository name**, set it to `HelixTest`, and save. Then update your local remote:  
`git remote set-url origin https://github.com/SynapticFour/HelixTest.git`

---

**Synaptic Four** — Built with ❤️ for the open science community. Implementing GA4GH open standards for sovereign bioinformatics infrastructure. Proudly developed by individuals on the autism spectrum in Germany 🇩🇪 We build tools that are precise, thorough, and designed to work exactly as documented.  
© 2025 Synaptic Four · Licensed under [Apache-2.0](LICENSE).  
Contact: [contact@synapticfour.com](mailto:contact@synapticfour.com) · [synapticfour.com](https://synapticfour.com)

