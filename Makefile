# HelixTest — build and run conformance (no local stack)

.PHONY: help install test prove

# Live-stack crates need a running Ferrum/infra target. Same excludes as CI.
OFFLINE_TEST_FLAGS := --workspace --exclude api-tests --exclude auth-tests --exclude e2e-tests --exclude workflow-tests

help:
	@echo "HelixTest — conformance runner (Synaptic Four GA4GH stack)"
	@echo ""
	@echo "  make install   Build helixtest CLI (cargo build --release)"
	@echo "  make test      Offline unit tests (CI parity; no live stack)"
	@echo "  make prove     Zero-risk proof: offline tests + release CLI"
	@echo ""
	@echo "Live (needs a target): see docs/PROVE.md"
	@echo "HelixTest does not deploy servers. Start Ferrum or Demo first:"
	@echo "  cd ../Ferrum && make up"
	@echo "  helixtest --all --mode ferrum"

install:
	cargo build --release -p helixtest-cli
	@echo "Binary: target/release/helixtest"

test:
	cargo test $(OFFLINE_TEST_FLAGS)

# Zero-risk customer path. Live Ferrum proof: docs/PROVE.md
prove: test
	cargo build --release -p helixtest-cli
	@echo "HelixTest prove OK. Binary: target/release/helixtest"
