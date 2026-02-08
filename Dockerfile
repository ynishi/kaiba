# Multi-stage build for kaiba-server
# Usage: docker build -t kaiba-server .

# ---- Builder ----
FROM rust:1.93-slim AS builder
WORKDIR /app

RUN apt-get update && apt-get install -y pkg-config libssl-dev curl && rm -rf /var/lib/apt/lists/*

# Copy workspace manifests for dependency caching
COPY Cargo.toml Cargo.lock ./
COPY crates/kaiba/Cargo.toml crates/kaiba/Cargo.toml
COPY crates/kaiba-server/Cargo.toml crates/kaiba-server/Cargo.toml
COPY crates/kaiba-cli/Cargo.toml crates/kaiba-cli/Cargo.toml
COPY crates/kaiba-integration-discord/Cargo.toml crates/kaiba-integration-discord/Cargo.toml

# Dummy sources to build dependencies only (cache layer)
RUN mkdir -p crates/kaiba/src crates/kaiba-server/src crates/kaiba-cli/src crates/kaiba-integration-discord/src && \
    echo "pub fn _dummy() {}" > crates/kaiba/src/lib.rs && \
    echo "fn main() {}" > crates/kaiba-server/src/main.rs && \
    echo "fn main() {}" > crates/kaiba-cli/src/main.rs && \
    echo "fn main() {}" > crates/kaiba-integration-discord/src/main.rs && \
    cargo build --package kaiba-server --release 2>/dev/null || true

# Copy real sources and rebuild
COPY crates/ crates/
RUN touch crates/kaiba/src/lib.rs crates/kaiba-server/src/main.rs && \
    cargo build --package kaiba-server --release

# ---- Runtime ----
FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates libssl3 && rm -rf /var/lib/apt/lists/* && \
    adduser --disabled-password --gecos "" --home /app appuser
WORKDIR /app
COPY --from=builder /app/target/release/kaiba-server .
RUN chown appuser:appuser ./kaiba-server
USER appuser

ENV PORT=8080
EXPOSE 8080
CMD ["./kaiba-server"]
