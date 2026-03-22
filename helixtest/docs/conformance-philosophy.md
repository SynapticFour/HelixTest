# Conformance philosophy & non-goals

HelixTest is a **GA4GH conformance and interoperability** suite. It is **spec-first**: tests are justified by published GA4GH API behaviour (schemas, lifecycles, error semantics, checksums, security expectations where the spec defines them).

## What HelixTest is

- **Boolean / structural conformance** — Does the implementation expose valid responses and state transitions?
- **Levels and weighted scores** — Summarise how far a deployment gets through the defined ladder; `--fail-level` gates on those levels, not on runtime.
- **CI-friendly** — Deterministic ordering, JSON reports, clear skips when a profile disables a module.

## What HelixTest is not

- **Not a performance benchmark framework** — Wall-clock duration, throughput, median latency, or “plain vs Crypt4GH speed” are **not** pass/fail criteria and do not affect compliance levels or scores.
- **Not a load-testing tool** — Timeouts exist only to bound **polling** for asynchronous lifecycle (WES/TES) so runs finish; they are not performance SLAs.

If you need throughput or latency regression gates, use a separate benchmark or demo repository (Apache-2.0 or similar) that can **call the same APIs** HelixTest validates.

## Lifecycle polling (WES / TES)

- Between polls, implementations may report **non-terminal** states for a long time. That is allowed by the API model.
- **Success** is defined by the **canonical terminal outcome** for that test (e.g. WES `COMPLETE` with expected outputs), together with **monotonic** state progression where HelixTest checks it (see `common::workflow` for WES).
- **WES** reporting `QUEUED` / `INITIALIZING` / `RUNNING` does **not** imply that **TES** (or any other backend) is already terminal; HelixTest does not require that coupling unless a **documented** cross-service test (e.g. mock-specific `e2e-tests`) asserts it.

## DRS vs heavy E2E

- **DRS** checks focus on **object shape**, **access methods**, **byte integrity**, **errors**, and **range** where applicable — conformance to the DRS contract.
- **Heavy** TRS → WES → … pipelines belong in **E2E** modules or the `e2e-tests` crate. Use **subset profiles** and `--only` to run a **narrower** path in PR CI (see [subset-profiles.md](subset-profiles.md)); that remains conformance-only, not faster == better.

## Crypt4GH

- Optional Ferrum HTTP paths stay **feature-gated** (`HELIXTEST_FEATURE_CRYPT4GH_*`). Local Crypt4GH tests assert **integrity and failure modes** (wrong key, corruption, truncation), not transfer speed.

## Reporting diagnostics

- Set `HELIXTEST_REPORT_DIAGNOSTICS=true` (or `1`) to add optional JSON fields such as **`suite_duration_ms`**. These are **diagnostics only** and are **excluded** from compliance level and score calculation unless a future, separately documented “observability mode” changes that (default today: off).

## Operator expectations

- Rough **resource** hints for profiles (RAM / disk order of magnitude) are in [subset-profiles.md](subset-profiles.md) and [ferrum.md](ferrum.md) so local and CI failures are easier to interpret — not requirements enforced by the suite.
