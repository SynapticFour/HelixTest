# Architecture Overview

HelixTest is a Rust workspace whose `helixtest` CLI drives a shared **framework** against live GA4GH-style endpoints (WES, DRS, TRS, TES, Beacon, htsget, auth, Crypt4GH, and cross-service E2E). It does **not** host those services: operators start Ferrum, ga4gh-infra, or another target, then point profiles/config at their URLs. Detailed crate layout, mode resolution, and reporting live in [`helixtest/docs/architecture.md`](helixtest/docs/architecture.md); ecosystem lifecycle notes are in [`docs/ECOSYSTEM.md`](docs/ECOSYSTEM.md).
