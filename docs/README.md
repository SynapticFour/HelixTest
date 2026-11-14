# HelixTest documentation

Every document under `docs/`, grouped as a tutorial, how-to guides, reference, and explanation. One line each.

The crate documents live under `helixtest/docs/`. They are not in this folder. The architecture page links the ones a reader of this index needs.

## Tutorial

| Document | What it is |
|----------|------------|
| [GETTING-STARTED.md](GETTING-STARTED.md) | Rust, `make prove`, and the commands against a service you already started. |
| [INSTALL.md](INSTALL.md) | Install the `helixtest` binary without cloning the suite. |
| [PROVE.md](PROVE.md) | What `make prove` runs offline, and where a live run is written. |

## How-to guides

| Document | What it is |
|----------|------------|
| [edge-profile.md](edge-profile.md) | The Ferrum edge profile: which interfaces run and which are skipped. |
| [ECOSYSTEM.md](ECOSYSTEM.md) | How to start and stop sibling repositories before a live run. |

## Reference

| Document | What it is |
|----------|------------|
| [FOR-EVALUATORS.md](FOR-EVALUATORS.md) | What a run checks, and what it does not certify. |
| [third-party-targets.md](third-party-targets.md) | The October 2026 runs against other servers. Not a ranking. |
| [DEPENDENCY.md](DEPENDENCY.md) | Why Dependabot and Renovate are off, and how pins are kept. |
| [IDENTITY.md](IDENTITY.md) | Who the command is for, and which jobs stay in other repositories. |

Files outside `docs/` that the same reading needs: [CHANGELOG.md](../CHANGELOG.md), [SECURITY.md](../SECURITY.md), [CONTRIBUTING.md](../CONTRIBUTING.md), [TESTING.md](../TESTING.md), [LICENSE](../LICENSE), [CITATION.cff](../CITATION.cff).

## Explanation

| Document | What it is |
|----------|------------|
| [ARCHITECTURE.md](ARCHITECTURE.md) | The workspace, and where the crate layout is written. |
| [README.md](README.md) | This index. |
