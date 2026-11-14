# Architecture

HelixTest is a Rust workspace whose `helixtest` CLI drives a shared framework against live GA4GH-style endpoints (WES, TES, DRS, TRS, Beacon, htsget, auth, Crypt4GH, and cross-service end-to-end tests). It does not host those services. An operator starts Ferrum, ga4gh-infra, or another target, then points profiles and config at their URLs.

GA4GH is a set of shared interfaces for genomic and health data. HelixTest is not GA4GH and is not certified by GA4GH.

The crate layout, mode resolution, and reporting are in [../helixtest/docs/architecture.md](../helixtest/docs/architecture.md). How sibling repositories start and stop is in [ECOSYSTEM.md](ECOSYSTEM.md).

Helix, the `helix` command around this suite, is a separate repository: [https://github.com/SynapticFour/Helix](https://github.com/SynapticFour/Helix). Signed evidence stays in [HELIOS](https://github.com/SynapticFour/HELIOS).
