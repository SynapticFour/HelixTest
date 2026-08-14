# HelixTest Scoring System

This document explains how compliance levels and scores are computed.

## Test outcomes: Pass, Fail, Skip

Each check has a `status` of **Pass**, **Fail**, or **Skip** (`TestStatus`). Skip means the check was not executed (feature off, missing URL, env not set). Skips are **not** passes.

- **Skip is excluded** from achieved level, weighted score, `--fail-level`, and `has_failures`.
- The table prints `SKIP <name>: <reason>` for skipped checks.
- `passed: true` is kept for JSON compatibility and is true **only** when `status == Pass`.

## Compliance Levels (0–5)

Each test is assigned a **compliance level** that reflects the kind of requirement it checks:

| Level | Meaning | Examples |
|-------|--------|----------|
| **0** | API reachable | Service responds 2xx or 401 (auth required). 404/405 fail. |
| **1** | Schema compliant | Response structure, required fields, enums match spec |
| **2** | Functional correctness | Lifecycle, outputs, checksums behave as expected |
| **3** | Interoperability | Cross-service flows (E2E pipeline, TRS→DRS→WES) |
| **4** | Security | HMAC JWT fixture (default Auth) or Passports (`--mode ferrum+infra`) |
| **5** | Robustness | Negative cases, corruption, wrong keys, 404 handling |

## Per-Service Achieved Level

A **service’s achieved level** is the highest level N such that **Level 0 was executed and passed**, and every executed (non-skip) test at each higher level that has tests also passed. Empty or skip-only levels in between do **not** block a higher N. If any executed test at level N fails, the service’s level stays at the last fully-green N.

- Example: WES has Level 0 and 1 tests passing, one Level 2 test failing → WES achieved level = 1.
- A suite with **only Level 5** tests (no executed Level 0) achieves **Level 0**. Local age checks therefore include an explicit Level 0 “library available” pass.
- Skip-only Level 4 does not prevent Level 5 from counting once Level 0 passed, and does not pin the service at 3.

## Overall Level

The **overall** (suite) level is the **minimum** of all per-service achieved levels **among services that executed at least one non-skip test**. A skip-only service does not pin overall level.

- So if WES is 2, DRS is 3, and TES is 1, overall level = 1.
- This reflects that the suite as a whole is only as strong as the weakest **executed** service.

## Weighted Score (0.0–1.0)

Each test has a **weight** (default 1.0; skips use 0). For a given service:

- Tests with `status == Skip` or `weight <= 0` are **omitted** (not treated as weight 1.0).
- **Service score** = (sum of weights of passed tests) / (sum of weights of remaining tests).
- **Overall score** = average of all service scores.

So:

- **1.0** = all executed tests passed.
- **0.0** = no executed tests passed, or only skips.
- Values in between reflect partial pass (e.g. 0.8 = 80% of weighted executed tests passed).

Used for:

- `--report scores` – outputs per-service level + score and overall level + score (JSON).
- Optional CI gates (e.g. require overall score ≥ 0.9).

## Fail Level (CLI)

`--fail-level N` means: exit with code 1 if the **overall level** is **below** N, even if every executed test passed. Skips do not count as failures and do not lower overall level by themselves.

- `--fail-level 3` → fail the run unless every **executed** service achieved at least level 3.

This is independent of the weighted score; it’s a level-based gate. The process also exits 1 if any test **Failed**.

## Coverage Summary

`--report coverage` outputs a matrix:

- **Per service**, for each **test category** (Schema, Lifecycle, Checksum, Interoperability, Security, Robustness, Other):
  - **Pass** – at least one executed test in that category and all passed.
  - **Fail** – at least one executed test in that category and at least one failed.
  - **Missing** – no executed tests in that category (including skip-only).

Categories come from each test’s `category` field and help you see which areas are covered or failing.

## Summary

| Concept | Definition |
|--------|------------|
| **Test status** | Pass, Fail, or Skip. Skip is not a pass. |
| **Test level** | 0–5, assigned per test (reachability → robustness). |
| **Service achieved level** | Highest N with executed L0 passing, then all executed tests at each higher populated level passing. L5-only → 0. |
| **Overall level** | Min of executed services’ achieved levels. |
| **Service score** | Weighted fraction of executed tests (skips and `weight <= 0` omitted). |
| **Overall score** | Average of service scores. |
| **Fail level** | CLI exits 1 if overall level &lt; N, or if any test Failed. |

---

*HelixTest by Synaptic Four — built for the open science community. © 2025 Synaptic Four · Apache-2.0.*
