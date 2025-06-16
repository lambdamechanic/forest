# --- Stage 0: cargo-chef + Rust toolchain
FROM lukemathwalker/cargo-chef:latest-rust-1 AS chef
WORKDIR /app

# --- Stage 1: compute dependency "recipe"
FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

# --- Stage 2: build deps, then the binary
FROM chef AS builder
COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json
COPY . .
RUN cargo build --release --bin forest

# --- Stage 3: tiny runtime
FROM debian:bookworm-slim
COPY --from=builder /app/target/release/forest /usr/local/bin/
ENTRYPOINT ["/usr/local/bin/forest"]
