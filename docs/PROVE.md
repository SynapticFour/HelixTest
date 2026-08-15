# Prove HelixTest without a running platform

`make prove` is the zero-risk path: **offline** workspace tests and a release CLI build. No Docker, no Ferrum.

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
