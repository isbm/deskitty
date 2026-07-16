.PHONY: dev release clean check test

help:
	@echo "dev       Build debug binary (unoptimized, with debug info)"
	@echo "release   Build optimized release binary"
	@echo "clean     Remove all build artifacts (cargo clean)"
	@echo "check     Format code and run clippy lints (auto-fix on failure)"
	@echo "test      Run tests with cargo-nextest"

dev:
	cargo build

release:
	cargo build --release

clean:
	cargo clean

check:
	cargo fmt --all
	cargo clippy --all-targets --all-features -- -D warnings || \
		cargo clippy --fix --all-targets --all-features --allow-dirty --allow-staged -- -D warnings

test:
	cargo nextest run
