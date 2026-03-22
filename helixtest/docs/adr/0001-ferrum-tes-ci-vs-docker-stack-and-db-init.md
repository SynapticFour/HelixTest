# ADR 0001: Ferrum — noop TES (CI / HelixTest defaults) vs Docker TES (full GA4GH stack) and DB init expectations

## Status

Accepted (documentation only; behaviour lives in the **Ferrum** repository).

## Context

- **HelixTest** is the **automated GA4GH conformance suite** in this repo. It is **not** Ferrum’s runtime configuration.
- **Ferrum** can run with different **TES backends**:
  - **noop TES** — lightweight; suitable for **CI** and quick HelixTest runs (TES endpoint present; minimal real task execution).
  - **Docker TES** — needed for **self-hosted** setups and **GA4GH-style demos** that must execute real containers and write outputs to mounted volumes.
- Operators sometimes **re-run `ferrum-init`** (or migrations) against an **existing Postgres volume** without `docker compose down -v`, leading to errors such as **“relation already exists”** or **partial migration state**.

## Decision

### 1) Split: “HelixTest / CI defaults” vs “full stack / GA4GH demo”

| Stack | TES backend (typical) | Use case |
|--------|------------------------|----------|
| **HelixTest + Ferrum in CI** | **noop TES** | Fast PR checks; `profiles/ferrum.toml` URLs stay valid; TES Level 0–2 behaviour depends on what noop implements. |
| **GA4GH demo / serious self-host** | **Docker TES** (or other real executor) | Real task lifecycle, bind mounts, network to DRS/WES helpers. |

**Naming (clarity):** In docs and runbooks, treat **“HelixTest default Ferrum run”** as *conformance against whatever TES backend Ferrum exposes on `TES_URL`* — in Synaptic Four CI that is intentionally **noop TES**. Demos that need **real** execution must switch Ferrum to **Docker TES** and document env; do not fork `Dockerfile.gateway` only to flip TES — prefer **compose profiles** or a **documented recipe** in the Ferrum repo.

### 2) Docker TES — environment variables (reference)

These are **Ferrum-side** settings (names as used in Ferrum docs / deployment; confirm against current Ferrum `docker-compose` and env schema):

| Variable | Role |
|----------|------|
| `FERRUM_TES_BACKEND` | Selects TES implementation (e.g. noop vs docker). |
| `FERRUM_TES_WORK_DIR` | Host/worker directory for task I/O. |
| `FERRUM_TES_EXTRA_BINDS` | Extra bind mounts into the TES executor (e.g. shared test-data). |
| `FERRUM_TES_DOCKER_NETWORK` | Attach TES containers to the same network as gateway/DRS. |
| `FERRUM_TES_EXTRA_HOSTS` | Extra `host:ip` mappings when services are not on default DNS. |
| `FERRUM_TES_DOCKER_PLATFORM` | Optional platform (e.g. `linux/amd64`) for cross-arch demos. |

**HelixTest** does not set these; it only calls `TES_URL`. Operators point `TES_URL` (or `HELIXTEST_PROFILE=ferrum` + overrides) at the gateway that already uses the chosen backend.

**Recommendation for Ferrum (implementation ticket):** Add a **Compose profile** (e.g. `docker-tes`) or a **small override file** `compose.docker-tes.yaml` so demos enable Docker TES without maintaining a fork of the gateway image Dockerfile.

### 3) `ferrum-init` / migrations — expectations

Until Ferrum **hardens** migrations (idempotency, explicit version table, or a safe **“already initialized”** guard):

- **Development / demo reset:** After schema or seed changes, assume a **clean Postgres volume** is required unless Ferrum documents otherwise. Typical pattern: `docker compose down -v` (or volume prune) before `ferrum-init` when you see duplicate-relation or inconsistent migration errors.
- **Production:** Follow Ferrum’s release notes for **forward-only** migrations; never assume `ferrum-init` is safe to re-run blindly on a live DB without Ferrum’s migration tooling.

**HelixTest** operators: if CI or local runs fail on **stale DB state**, reset the Ferrum Postgres volume per Ferrum docs — this is **not** a HelixTest bug.

## Consequences

- This repo documents **intent** and **operator expectations**; concrete Compose profiles and migration code belong in **Ferrum**.
- HelixTest **README / ferrum.md** link here so “why noop TES in CI” and “how to run Docker TES demos” stay explicit.

## References

- [Ferrum HelixTest integration](https://github.com/SynapticFour/Ferrum/blob/main/docs/HELIXTEST-INTEGRATION.md) (upstream; may evolve)
- [Testing Ferrum with HelixTest](../ferrum.md)
- [Conformance philosophy](../conformance-philosophy.md)

---

### Kurzfassung (DE)

- **HelixTest** = automatisierte GA4GH-Conformance-Suite (dieses Repo); **kein** Performance- oder TES-Backend-Selector.
- **CI:** Ferrum oft mit **noop TES** — schnell, für HelixTest-PRs ausreichend, sofern die TES-API die erwarteten Checks erfüllt.
- **Demos / Self-Host:** **Docker TES** mit dokumentierten Env-Variablen (`FERRUM_TES_*`); besser **Compose-Profile** in Ferrum als Fork von `Dockerfile.gateway`.
- **`ferrum-init` / Migrationen:** Bei bestehendem Postgres-Volume ohne sauberen Reset können „relation already exists“ o. Ä. auftreten — bis Ferrum idempotenter wird: für Dev oft **`docker compose down -v`** vor erneutem Init; Produktion nur nach Ferrum-Migrationsdoku.
