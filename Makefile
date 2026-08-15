# HelixTest — build and run conformance (no local stack)

.PHONY: help install test prove

help:
	@echo "HelixTest — conformance runner (Synaptic Four GA4GH stack)"
	@echo ""
	@echo "  make install   Build helixtest CLI (cargo build --release)"
	@echo "  make test      Run workspace unit tests"
	@echo "  make prove     Zero-risk proof: tests + release CLI (no Docker)"
	@echo ""
	@echo "Live (needs a target): see docs/PROVE.md"
	@echo "HelixTest does not deploy servers. Start Ferrum or Demo first:"
	@echo "  cd ../Ferrum && make up"
	@echo "  helixtest --all --mode ferrum"

install:
	cargo build --release -p helixtest-cli
	@echo "Binary: target/release/helixtest"

test:
	cargo test --workspace

# Zero-risk customer path. Live Ferrum proof: docs/PROVE.md
prove: test
	cargo build --release -p helixtest-cli
	@echo "HelixTest prove OK. Binary: target/release/helixtest"
