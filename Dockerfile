# Build stage
FROM rust:1.75 AS builder

WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY src ./src
COPY benches ./benches
COPY tests ./tests
COPY proto ./proto

RUN cargo build --release --all-features

# Runtime stage
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y \
    libssl3 \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/sigma-rs /usr/local/bin/sigma-rs

ENTRYPOINT ["/usr/local/bin/sigma-rs"]