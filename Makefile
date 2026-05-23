# SPDX-License-Identifier: MIT OR Apache-2.0
# Russell — development targets.

.PHONY: build check test lint fmt clean install dev-install

build:
	cargo build

check:
	cargo check

test:
	cargo test --workspace

lint:
	cargo clippy --workspace --all-targets -- -D warnings

fmt:
	cargo fmt --check

clean:
	cargo clean

install:
	./install.sh

dev-install:
	./install.sh --dev

# Run sentinel once + view results
sentinel:
	@echo "=== running sentinel-once ==="
	cargo run -- sentinel-once
	@echo ""
	@echo "=== most recent 10 samples ==="
	cargo run -- list --limit 10

# Run jack with an optional note
jack:
	cargo run -- jack $(if $(note),--note "$(note)",)