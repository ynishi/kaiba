.PHONY: check build test publish-cli publish-cli-dry release-minor-dry release-minor

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

# Release management
# Increments minor version (0.1.x -> 0.2.0), creates git tag, updates changelog
release-minor-dry:
	cargo release minor --workspace

release-minor:
	cargo release minor --workspace --execute
