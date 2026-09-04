# Prove HelixTest without a running platform

`make prove` is the zero-risk path: **offline** workspace tests (including a mock GA4GH DRS, not Ferrum) and a release CLI build. No Docker, no Ferrum.

```bash
git clone https://github.com/SynapticFour/HelixTest.git && cd HelixTest
make prove
```

That is HelixTest as a product: the suite compiles and its own unit tests pass. GitHub Actions on `main` / PRs run the same `make prove` command.

`make test` / `make prove` **exclude** the live-stack crates (`api-tests`, `auth-tests`, `e2e-tests`, `workflow-tests`). Those crates call `framework::run_all` against a running target. Hitting them without a stack is not a customer proof.

## Live proof (needs a target you control)

HelixTest never starts servers. Demo-open auth is **not** the customer path.

```bash
# Terminal 1 — Ferrum with auth required (see Ferrum docs/IDENTITY.md)
cd ../Ferrum && make up

# Terminal 2
helixtest --all --mode ferrum
```

Passports / ADS:

```bash
cd ../Ferrum-GA4GH-Demo && make up-with-infra
helixtest --all --mode ferrum+infra --profile ferrum-infra
```

Results are not GA4GH certification. Known gaps: [helixtest/docs/known-limitations.md](../helixtest/docs/known-limitations.md).

## `--start-compose` / `--start-ferrum` (default compose file)

**Do not rely on** `helixtest --start-ferrum` / `--start-compose` with the default file `helixtest/docker/docker-compose.yml` until those images are replaced. See D2 in Helix `docs/DECISIONS.md` and `INVENTORY.md`.

That file lists `ghcr.io/example/mock-{wes,tes,drs,trs,beacon,oidc}:latest`. Checked **2026-09-04** with Docker **29.7.2**: `docker manifest inspect <image>` returned `manifest unknown` for every tag. (`docker pull --dry-run` is not available on this client.)

Re-check (human with Docker):

```bash
docker manifest inspect ghcr.io/example/mock-wes:latest
# expected while placeholders remain: errors with "manifest unknown"
```

Offline proof stays `make prove` (in-process mock DRS, not this compose file). Live proof is a stack **you** start (Ferrum `make up`, Demo, or `--compose-file` pointing at images you can pull).
