# Getting started

HelixTest checks a service that is already running. It does not start one. The command in this repository is `helixtest`.

## Prerequisites

Rust 1.91.1, pinned in `rust-toolchain.toml`. The workspace `rust-version` is 1.88. A binary install without cloning is [INSTALL.md](INSTALL.md).

## In this repository

```bash
git clone https://github.com/SynapticFour/HelixTest.git && cd HelixTest
make prove
```

You should see the line `HelixTest prove OK. Binary: target/release/helixtest`.

`make prove` runs the workspace tests, an SPDX check on first-party Rust files, and a release build of the CLI. Four tests that need a live target are ignored. `make test-live` runs them. Continuous integration on `main` and on pull requests runs `make prove`. It runs `make test-live` only when someone starts that job by hand. That job does not start a server.

## Against a service you already started

Start the service first. This tool does not start it.

[Ferrum](https://github.com/SynapticFour/Ferrum), after `make up` in that repository:

```bash
helixtest --all --mode ferrum
helixtest --all --mode ferrum --only drs
```

Any DRS, not only Ferrum. Set `PORT` to the port you started:

```bash
DRS_URL=http://127.0.0.1:$PORT helixtest --all --mode generic --only drs --profile ga4gh-drs --report json
```

Ferrum and [ga4gh-infra](https://github.com/SynapticFour/ga4gh-infra) together, after `make up-with-infra` in [Ferrum-GA4GH-Demo](https://github.com/SynapticFour/Ferrum-GA4GH-Demo):

```bash
helixtest --all --mode ferrum+infra --profile ferrum-infra
```

| Command | Purpose |
|---------|---------|
| `helixtest --all --mode ferrum` | Full suite against a Ferrum demo stack you started |
| `helixtest --all --mode ferrum+infra --profile ferrum-infra` | Co-deploy broker and Passport-on-DRS |
| `helixtest --all --mode ferrum+infra --profile ferrum-infra-pilot` | Same against Ferrum `make up-pilot-local` on port 8080 |
| `helixtest --all --profile edge` | Ferrum edge binary: DRS, Beacon, htsget, Crypt4GH. WES, TES, TRS, and Passports are skipped. See [edge-profile.md](edge-profile.md) |
| `helixtest --all --only wes --mode ferrum` | One service |
| `helixtest --help` | Flags, report formats, fail levels |

The 2026-10 runs against Sapporo, Funnel, the Starter Kit DRS, and beacon2-pi-api are in [third-party-targets.md](third-party-targets.md). That page does not add a profile for those servers.

Exit code `0` means the suite met `--fail-level`. Exit code `1` means failures, or a usage error. `--report table` is the human report. `--report json` is for a job. A Beacon 404 prints the URL and the expected status: start the target before running again.

Live commands are also in [PROVE.md](PROVE.md). The VERIFY command line around this suite is [Helix](https://github.com/SynapticFour/Helix). A GitHub Action that downloads release binaries is [helixtest-action](https://github.com/SynapticFour/helixtest-action). Its default binaries are HelixTest `v0.1.3`. That action does not start Ferrum.
