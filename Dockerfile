# ── Stage 1: Build ─────────────────────────────────────────────────────────
FROM rust:1.93-bookworm AS builder

# Install the MUSL target + linker toolchain
RUN rustup target add x86_64-unknown-linux-musl \
 && apt-get update \
 && apt-get install -y musl-tools \
 && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Cache dependencies first (dummy main so cargo can resolve + compile deps)
COPY Cargo.toml Cargo.lock ./
RUN mkdir src && echo "fn main() {}" > src/main.rs
RUN cargo build --release --target x86_64-unknown-linux-musl
RUN rm -rf src

# Build the real binary
COPY src ./src
RUN touch src/main.rs \
 && cargo build --release --target x86_64-unknown-linux-musl

# ── Stage 2: Runtime ───────────────────────────────────────────────────────
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/x86_64-unknown-linux-musl/release/beacon /usr/local/bin/beacon

# Default command to start the server
CMD ["beacon", "serve"]
