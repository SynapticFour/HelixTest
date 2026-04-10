# Testing Strategy

This repository currently has limited automated tests.

## Minimum local checks before merging

- Run formatting checks
- Run linting checks
- Run type checks (if applicable)
- Run the repository quality gate workflow in CI

## Near-term test plan

1. Add smoke tests for critical user paths.
2. Add regression tests for every production bugfix.
3. Add at least one deterministic CI test suite per runtime stack.

## PR requirement

Any non-trivial behavior change should include tests or a documented reason why tests are not feasible.
