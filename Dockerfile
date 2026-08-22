FROM lukemathwalker/cargo-chef:latest-rust-1-bookworm AS chef
WORKDIR /app

FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder
RUN cargo install sqlx-cli --no-default-features --features sqlite
COPY --from=planner /app/recipe.json recipe.json
# Build dependencies - this is the caching Docker layer!
RUN cargo chef cook --release --recipe-path recipe.json
# Build application
COPY . .
ENV SQLX_OFFLINE=true
RUN cargo build --release --bin vardy

# We do not need the Rust toolchain to run the binary!
FROM debian:bookworm-slim AS runtime
# The binary links OpenSSL dynamically (reqwest/native-tls) and needs CA roots
# for outbound HTTPS; bookworm-slim ships neither.
RUN apt-get update \
    && apt-get install -y --no-install-recommends libssl3 ca-certificates \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY --from=builder /usr/local/cargo/bin/sqlx /usr/local/bin/sqlx
COPY --from=builder /app/migrations ./migrations
COPY --from=builder /app/templates ./templates
COPY --from=builder /app/static ./static
COPY --from=builder /app/target/release/vardy /usr/local/bin
ENV DATABASE_URL=sqlite:test.db
RUN sqlx database create
RUN sqlx migrate run
ENTRYPOINT ["/usr/local/bin/vardy"]
