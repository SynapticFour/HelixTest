# Known limitations

Intentional remaining constraints after closing the 14 Aug 2026 follow-up gaps.

## Serial HTTP

WES/TES cases stay **serial** so the target is not overloaded. Independent services still run one after another. A down host fails faster than the old 5×60s retry (5s connect, two GET attempts). Parallel service checks are out of scope.

## Live-stack cargo tests

`api-tests`, `auth-tests`, `e2e-tests`, and `workflow-tests` now call `framework::run_all` (same checks as `helixtest --all`). They still need a running stack and stay **excluded** from default CI / `scripts/hooks/ci-check.sh`. In-process age checks live in `crypt4gh-tests` and run in CI.

## Africa / Infra modes

`--only africa` / `--only infra` in generic `--all` are recorded as skipped with “use `--mode ferrum-africa` or `--mode ferrum+infra`”. Those suites are not mixed into the default ladder.

## jsonschema `'static` leak

jsonschema 0.17 compiles from `&'static Value`. Each official schema is `Box::leak`’d **once** when first compiled (`OnceCell`). A leak-free API needs jsonschema ≥0.26. Left on 0.17 to avoid pulling reqwest 0.12 alongside the workspace’s reqwest 0.11. The old per-call leak in `validate_json_against` is gone (that helper was removed).

## `once_cell`

`ga4gh_schemas` uses `once_cell::OnceCell::get_or_try_init`. `std::sync::OnceLock::get_or_try_init` is still unstable (`once_cell_try`).

---

*HelixTest by Synaptic Four — Apache-2.0.*
