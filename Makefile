# HelixTest — build and run conformance (no local stack)

.PHONY: help install test

help:
	@echo "HelixTest — conformance runner (Synaptic Four GA4GH stack)"
	@echo ""
	@echo "  make install   Build helixtest CLI (cargo build --release)"
	@echo "  make test      Run workspace unit tests"
	@echo ""
	@echo "HelixTest does not deploy servers. Start Ferrum or Demo first:"
	@echo "  cd ../Ferrum && make up"
	@echo "  helixtest --all --mode ferrum"

install:
	cargo build --release -p helixtest-cli
	@echo "Binary: target/release/helixtest"

test:
	cargo test --workspace
