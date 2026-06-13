# SynapticFour GA4GH stack

Five repositories implement a coherent on-premises GA4GH platform. This file is **mirrored** in each repo so readers can navigate between projects without relearning structure.

**You are here:** [HelixTest](https://github.com/SynapticFour/HelixTest) — GA4GH conformance and integration tests (`helixtest` CLI).

## Repositories

| Repository | Role | License |
|------------|------|---------|
| [ga4gh-infra](https://github.com/SynapticFour/ga4gh-infra) | OIDC broker, visa registry, DUO, ADS, service registry | Apache-2.0 |
| [Ferrum](https://github.com/SynapticFour/Ferrum) | DRS, WES, TES, TRS, Beacon, htsget, Crypt4GH gateway | BUSL-1.1 |
| [Ferrum-Lab-Kit](https://github.com/SynapticFour/Ferrum-Lab-Kit) | `lab-kit` profiles, compose generation, edge install | BUSL-1.1 |
| [Ferrum-GA4GH-Demo](https://github.com/SynapticFour/Ferrum-GA4GH-Demo) | `./run` benchmark and co-deploy scenarios | Apache-2.0 |
| **HelixTest** | `helixtest` conformance suite (this repo) | Apache-2.0 |

## Ownership boundaries

| Layer | Owner | Notes |
|-------|--------|--------|
| Identity | **ga4gh-infra** | Broker, visas, DUO, ADS, service registry |
| Data/compute | **Ferrum** | DRS, WES/TES, TRS, Beacon; built-in passports in standalone mode |
| Deployment | **Ferrum-Lab-Kit** | Selective GA4GH surfaces for labs; does not fork Ferrum |
| Demo/benchmark | **Ferrum-GA4GH-Demo** | Reproducible GIAB benchmark; optional `--with-infra` |
| Conformance | **HelixTest** | Automated API and workflow tests |

HelixTest **validates** implementations; it does not ship GA4GH services. Ferrum runs this suite in CI. See [helixtest/docs/ferrum.md](helixtest/docs/ferrum.md).

## Default co-deploy ports

| Service | Standalone Ferrum | Co-deploy (demo / lab) |
|---------|-------------------|-------------------------|
| Ferrum gateway | 8080 | 18080 (demo) or 8080 (lab) |
| AAI broker | — | 8180 |
| Visa registry | — | 8181 |
| DUO | — | 8182 |
| Service registry | — | 8183 |
| ADS | — | 8190 |
| mock-idp | — | 9100 |

## Quick starts

**Benchmark + co-deploy (demo):**

```bash
export FERRUM_SRC=/path/to/Ferrum
export GA4GH_INFRA_SRC=/path/to/ga4gh-infra
cd Ferrum-GA4GH-Demo && ./run --with-infra
```

**Field edge + infra (lab):**

```bash
cd Ferrum-Lab-Kit && ./install-edge.sh --with-infra
```

**Conformance (this repo):**

```bash
helixtest --all --mode ferrum
helixtest --all --mode ferrum+infra --profile ferrum-infra
```

## Documentation map

| Topic | Document |
|-------|----------|
| Ferrum ↔ ga4gh-infra wiring | [Ferrum GA4GH-INFRA-INTEGRATION.md](https://github.com/SynapticFour/Ferrum/blob/main/docs/GA4GH-INFRA-INTEGRATION.md) |
| Ferrum testing guide | [helixtest/docs/ferrum.md](helixtest/docs/ferrum.md) |
| Demo compose merge order | [Ferrum-GA4GH-Demo architecture.md](https://github.com/SynapticFour/Ferrum-GA4GH-Demo/blob/main/docs/architecture.md) |
| Lab co-deploy profiles | [field-edge+infra.toml](https://github.com/SynapticFour/Ferrum-Lab-Kit/blob/main/config/profiles/field-edge+infra.toml) |
| Africa-Mode (SQLite) | [ga4gh-infra AFRICA-DEPLOYMENT](https://github.com/SynapticFour/ga4gh-infra/blob/main/docs/AFRICA-DEPLOYMENT.md) |

## CI

GitHub Actions runs `cargo test`, ARM64 builds, and dependabot smoke on `main`. Ferrum and Ferrum-Lab-Kit invoke HelixTest in their own CI workflows.
