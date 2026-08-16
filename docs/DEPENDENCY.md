# Dependency care (ambassador)

Dependabot and Renovate are **off by choice**. Pin care is:

- `Cargo.lock` committed; bumps via reviewed PRs
- GitHub **Dependency Review** where enabled
- MSRV **1.88** (`Cargo.toml` `rust-version`); this checkout’s `rust-toolchain.toml` is **1.91.1** to match Ferrum/Lab-Kit/ga4gh-infra/Solum

There is no Dependabot smoke job on `main`.
