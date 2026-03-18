# HelixTest Scoring System

This document explains how compliance levels and scores are computed.

## Compliance Levels (0–5)

Each test is assigned a **compliance level** that reflects the kind of requirement it checks:

| Level | Meaning | Examples |
|-------|--------|----------|
| **0** | API reachable | Service responds to a basic request |
| **1** | Schema compliant | Response structure, required fields, enums match spec |
| **2** | Functional correctness | Lifecycle, outputs, checksums behave as expected |
| **3** | Interoperability | Cross-service flows (E2E pipeline, TRS→DRS→WES) |
| **4** | Security | Auth tokens, scopes, expiry (e.g. GA4GH Passports) |
| **5** | Robustness | Negative cases, corruption, wrong keys, 404 handling |

## Per-Service Achieved Level

A **service’s achieved level** is the highest level N such that **every** test at level N for that service **passed**. If any test at level N fails, the service’s level is below N (and we stop raising N).

- Example: WES has Level 0 and 1 tests passing, one Level 2 test failing → WES achieved level = 1.
- So: **achieved level = “max level where all tests at that level pass”.**

## Overall Level

The **overall** (suite) level is the **minimum** of all per-service achieved levels.

- So if WES is 2, DRS is 3, and TES is 1, overall level = 1.
- This reflects that the suite as a whole is only as strong as the weakest service.

## Weighted Score (0.0–1.0)

Each test has a **weight** (default 1.0). For a given service:

- **Service score** = (sum of weights of passed tests) / (sum of weights of all tests).
- **Overall score** = average of all service scores.

So:

- **1.0** = all tests passed.
- **0.0** = no tests passed.
- Values in between reflect partial pass (e.g. 0.8 = 80% of weighted tests passed).

Used for:

- `--report scores` – outputs per-service level + score and overall level + score (JSON).
- Optional CI gates (e.g. require overall score ≥ 0.9).

## Fail Level (CLI)

`--fail-level N` means: exit with code 1 if the **overall level** is **below** N, even if every test passed. So:

- `--fail-level 3` → fail the run unless every service achieved at least level 3.

This is independent of the weighted score; it’s a level-based gate.

## Coverage Summary

`--report coverage` outputs a matrix:

- **Per service**, for each **test category** (Schema, Lifecycle, Checksum, Interoperability, Security, Robustness, Other):
  - **Pass** – at least one test in that category and all passed.
  - **Fail** – at least one test in that category and at least one failed.
  - **Missing** – no tests in that category.

Categories come from each test’s `category` field and help you see which areas are covered or failing.

## Summary

| Concept | Definition |
|--------|------------|
| **Test level** | 0–5, assigned per test (reachability → robustness). |
| **Service achieved level** | Max level N such that all tests at level N for that service passed. |
| **Overall level** | Min of all service achieved levels. |
| **Service score** | Weighted fraction of passed tests (0.0–1.0). |
| **Overall score** | Average of service scores. |
| **Fail level** | CLI exits 1 if overall level &lt; N. |

---

*HelixTest by Synaptic Four — built for the open science community. © 2025 Synaptic Four · Apache-2.0.*
