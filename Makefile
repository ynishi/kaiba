.PHONY: check build test publish-cli publish-cli-dry

# Development
check:
	cargo check --workspace

build:
	cargo build --workspace

test:
	cargo test --workspace

# Publishing
# - kaiba-cli: Published to crates.io
# - kaiba (server): Shuttle-based, GitHub only (not published to crates.io)
publish-cli-dry:
	cargo publish -p kaiba-cli --dry-run

publish-cli:
	cargo publish -p kaiba-cli
