# HelixTest

HelixTest checks a service that is already running against GA4GH interfaces, a shared set of interfaces for genomic and health data. The command is `helixtest`.

For a team that runs that service and wants the check from outside it.

**Status: Early use.** Registry: HELIXTEST-MATURITY-001. Early use is not general availability and not a certification. HelixTest checks Synaptic Four's own products. The repository shows the status of each version.

[Helix](https://github.com/SynapticFour/Helix) is this suite as the `helix` command. [HELIOS](https://github.com/SynapticFour/HELIOS) stays signed evidence for a file already on disk. HelixTest does not sign evidence and does not start a server.

## What it does

- It points at a GA4GH API that is already running, including Ferrum. It does not start that API. Registry: TOOL-HELIXTEST-USE-001.
- It checks behaviour against the interface description, independent of which program implements it. Ferrum is a reference target, not a dependency.
- `make prove` runs the workspace tests, an SPDX check, and a release build of `helixtest`. Four tests that need a live target are ignored. `make test-live` runs them only when someone asks, and it does not start a server. Registry: TOOL-HELIXTEST-STATUS-001.
- A passing run is this project's own check. It is not GA4GH certification.

## Limits

- Early use is not general availability and not a certificate. Registry: HELIXTEST-MATURITY-001.
- It does not start Ferrum, ga4gh-infra, or any other server.
- Runs in October 2026 against other vendors' servers showed that dedicated profiles are needed. No comparison or ranking follows, and they are not evidence that an implementation conforms. Registry: HELIXTEST-COMPARE-001. The write-up is [docs/third-party-targets.md](docs/third-party-targets.md).
- Profiles for servers from other vendors are in progress. Registry: HELIX-OPEN-001. `profiles/ga4gh-drs.toml` on this branch only selects DRS checks.
- The latest published tag is `v0.1.4`; `git rev-parse v0.1.4^{commit}` prints its commit. The tag is the release lineage. The default branch can be ahead of it.
- MII and KDS checks live in Ferrum and Ferrum-Lab-Kit. This suite stays on GA4GH interfaces.
- This page describes technical behaviour. It is not legal advice and it does not by itself provide a regulatory certificate.

## Quick start

Prerequisites: Rust 1.91.1, pinned in `rust-toolchain.toml`. The workspace minimum is Rust 1.88.

```bash
make prove
```

You should see the line `HelixTest prove OK. Binary: target/release/helixtest`.

Against a service you already started: [Ferrum](https://github.com/SynapticFour/Ferrum), then `helixtest --all --mode ferrum` from this repository. This tool does not start that service. The commands are in [docs/GETTING-STARTED.md](docs/GETTING-STARTED.md).

## Documentation

Every file under `docs/` is listed in [docs/README.md](docs/README.md). The groups are a tutorial, how-to guides, reference, and explanation.

### Tutorial

- [Getting started](docs/GETTING-STARTED.md)
- [Install without cloning](docs/INSTALL.md)
- [What prove runs](docs/PROVE.md)

### How-to guides

- [Edge profile](docs/edge-profile.md)
- [Sibling repositories](docs/ECOSYSTEM.md)

### Reference

- [For evaluators](docs/FOR-EVALUATORS.md)
- [Third-party targets](docs/third-party-targets.md)
- [Changelog](CHANGELOG.md)

### Explanation

- [Architecture](docs/ARCHITECTURE.md)

## Licence

HelixTest is free to use under the Apache License 2.0, and you can read the source. Registry: TOOL-HELIXTEST-LICENSE-001. Not legal advice. The file is [LICENSE](LICENSE).

## Cite

Use [CITATION.cff](CITATION.cff). Prefer that file over a hand-copied line.

## Website

- Open tools: [https://synapticfour.com/en/offene-werkzeuge/](https://synapticfour.com/en/offene-werkzeuge/)
- Technical profile: [https://synapticfour.com/en/technical/code/](https://synapticfour.com/en/technical/code/)

The same pages exist in German (`/de/offene-werkzeuge/`, `/de/technik/code/`) and in French (`/fr/offene-werkzeuge/`, `/fr/technique/code/`).

The map of repositories: [profile/README.md](https://github.com/SynapticFour/.github/blob/main/profile/README.md).

Contact: [contact@synapticfour.com](mailto:contact@synapticfour.com)
