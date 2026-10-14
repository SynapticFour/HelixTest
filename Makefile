# HelixTest — build and run conformance (no local stack)

.PHONY: help install test test-live prove spdx-check

help:
	@echo "HelixTest — conformance runner (Synaptic Four GA4GH stack)"
	@echo ""
	@echo "  make install   Build helixtest CLI (cargo build --release)"
	@echo "  make test      Workspace tests. Live-service tests are ignored and do not call localhost."
	@echo "  make test-live Ignored live-service tests. A target must already be listening."
	@echo "  make prove     Zero-risk proof: workspace tests + SPDX + release CLI"
	@echo ""
	@echo "Live (needs a target): see docs/PROVE.md and TESTING.md"
	@echo "HelixTest does not deploy servers. Start Ferrum or Demo first:"
	@echo "  cd ../Ferrum && make up"
	@echo "  helixtest --all --mode ferrum"

install:
	cargo build --release -p helixtest-cli
	@echo "Binary: target/release/helixtest"

# Includes api-tests, auth-tests, e2e-tests, and workflow-tests. Those four
# tests are #[ignore] because they call a live target. `cargo test` stays green
# without services. The slow HTTP timeout test in helixtest-common stays ignored.
test:
	cargo test --workspace

# Runs only the ignored tests in the live-service crates. Does not run
# helixtest-common's slow timeout test. Assertions are unchanged.
test-live:
	cargo test -p api-tests -p auth-tests -p e2e-tests -p workflow-tests -- --ignored --test-threads=1

# Zero-risk customer path. Live Ferrum proof: docs/PROVE.md
prove: test
	$(MAKE) spdx-check
	cargo build --release -p helixtest-cli
	@echo "HelixTest prove OK. Binary: target/release/helixtest"

spdx-check:
	python3 scripts/spdx-rs.py . --license Apache-2.0 --check
