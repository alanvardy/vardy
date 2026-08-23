FROM lukemathwalker/cargo-chef:latest-rust-1-bookworm AS chef
WORKDIR /app

FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder
ARG TAILWIND_VERSION=v4.3.3
ARG TARGETARCH
ARG TAILWIND_SHA256_AMD64=dc61b3ac6b8c9ca874c0cc4c57b2409791a64c5540404ca5f5367360babc313a
ARG TAILWIND_SHA256_ARM64=55fd0b241214eff3de1e8ee4f22796662f2d2e7a49bcfca7477cfd0bac398195
RUN apt-get update \
    && apt-get install -y --no-install-recommends curl ca-certificates \
    && rm -rf /var/lib/apt/lists/*
RUN cargo install sqlx-cli --no-default-features --features sqlite
COPY --from=planner /app/recipe.json recipe.json
# Build dependencies - this is the caching Docker layer!
RUN cargo chef cook --release --recipe-path recipe.json
# Build application
COPY . .
# Rebuild CSS from source inside the image (overwrites the committed artifact).
# scripts/ is excluded by .dockerignore, so build-css.sh is inlined here.
# Pick the pinned CLI binary matching the target architecture (Fly amd64
# builders use linux-x64; arm64 hosts would otherwise run x64 under Rosetta,
# which cannot execute the Bun-based standalone CLI).
RUN set -eux; \
    case "$TARGETARCH" in \
        amd64) asset=tailwindcss-linux-x64; sha=$TAILWIND_SHA256_AMD64 ;; \
        arm64) asset=tailwindcss-linux-arm64; sha=$TAILWIND_SHA256_ARM64 ;; \
        *) echo "unsupported arch: $TARGETARCH" >&2; exit 1 ;; \
    esac; \
    curl -fsSL -o /usr/local/bin/tailwindcss \
        "https://github.com/tailwindlabs/tailwindcss/releases/download/${TAILWIND_VERSION}/${asset}"; \
    echo "${sha}  /usr/local/bin/tailwindcss" | sha256sum -c -; \
    chmod +x /usr/local/bin/tailwindcss; \
    tailwindcss -i css/site.css -o static/site.css --minify
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
ENV DATABASE_URL=sqlite:data/vardy.db
ENTRYPOINT ["/usr/local/bin/vardy"]
