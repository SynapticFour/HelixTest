# For evaluators

What this tree checks, and what a run is not. Not a sales brief. Not legal advice. Not a certification.

## What a run is

HelixTest is a Rust suite. The command is `helixtest`. You point it at a GA4GH API that is already running, including Ferrum. GA4GH is a set of shared interfaces for genomic and health data. The check is of behaviour against that description, independent of which program implements it. Ferrum is a reference target, not a dependency.

`make prove` checks this repository with no server: workspace tests, an SPDX check, and a release build of the CLI. Four tests that need a live target are ignored. `make test-live` runs them, and only when someone asks. A passing run is this project's own check. It is not an official GA4GH certification. Registry: TOOL-HELIXTEST-STATUS-001, HELIXTEST-MATURITY-001.

The public stage is Early use. Early use is not general availability. The published release tag `v0.1.3` peels to `1832c043e1679ec283cb2113510ee33684317cce`. The default branch this page was written from is `1926dbdd7c70985a3ab3b7b623a782e05f994453`, which `git describe` reports as `v0.1.3-35-g1926dbd`. The tag is the release lineage. It is not that commit.

## What it checks

- API contract tests for WES, TES, DRS, TRS, and Beacon v2
- Workflow execution tests for CWL, WDL, and Nextflow via WES
- Cross-service tests spanning TRS, DRS, WES, TES, and Beacon
- Authorization: the default suite uses an HMAC-SHA256 JWT fixture with a shared secret. GA4GH Passports and OIDC are exercised in `--mode ferrum+infra`
- Co-deploy tests via `--mode ferrum+infra` and profile `ferrum-infra`
- Cryptographic tests: local checks use age, not Crypt4GH containers. Optional Ferrum HTTP Crypt4GH rewrap and decrypt is gated by an environment variable. Compose files live under `helixtest/docker/`

Ferrum-specific notes: [../helixtest/docs/ferrum.md](../helixtest/docs/ferrum.md). Continuous integration uses a noop TES. Docker TES is in [../helixtest/docs/adr/0001-ferrum-tes-ci-vs-docker-stack-and-db-init.md](../helixtest/docs/adr/0001-ferrum-tes-ci-vs-docker-stack-and-db-init.md).

## What it is not

- It does not start Ferrum, ga4gh-infra, or any other server. Registry: TOOL-HELIXTEST-USE-001.
- It is not [HELIOS](https://github.com/SynapticFour/HELIOS). HELIOS writes signed evidence for a file already on disk.
- It is not the [Helix](https://github.com/SynapticFour/Helix) `helix` binary. Helix is this suite as that command.
- Test results are not official GA4GH certification.
- Runs in October 2026 against other vendors' servers showed that dedicated profiles are needed. No comparison or ranking follows from them, and they are not evidence that an implementation conforms. Registry: HELIXTEST-COMPARE-001. The write-up is [third-party-targets.md](third-party-targets.md).
- `profiles/ga4gh-drs.toml` on this branch only selects DRS checks. It does not describe Sapporo, Funnel, Starter Kit DRS, or beacon2-pi-api. Tag `v0.1.3` did not include that profile file. A first attempt in 2026-10 ran HelixTest `v0.1.3` against pinned images of those servers and produced failures. Part of those failures are tool issues: WES submit as JSON where the vendored schema allows only multipart, a TES MINIMAL view checked as a full task, DRS cases that require fixture `test-object-1`, and Beacon checks that POST `/query`. Those failures are not a ranking and not target non-conformance by themselves.
- Profiles for servers from other vendors are in progress. Registry: HELIX-OPEN-001.
- MII and KDS checks live in Ferrum and Ferrum-Lab-Kit. This suite stays on GA4GH interfaces.
- This page is not legal advice and does not by itself provide regulatory certification.

Known limits of the runner: [../helixtest/docs/known-limitations.md](../helixtest/docs/known-limitations.md).

## Contact

[contact@synapticfour.com](mailto:contact@synapticfour.com)
