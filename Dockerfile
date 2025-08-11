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
    && rm -rf /var/lib/apt/lists/* \
    && useradd -r -u 1001 -g root -s /bin/false sigma \
    && mkdir -p /app/rules \
    && chown -R sigma:root /app

COPY --from=builder /app/target/release/sigma-rs /usr/local/bin/sigma-rs

USER sigma

WORKDIR /app

ENTRYPOINT ["/usr/local/bin/sigma-rs"]