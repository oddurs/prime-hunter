# Multi-stage Docker build for darkreach
#
# Stage 1: Build the Rust release binary (with GMP)
# Stage 2: Minimal runtime image with binary
#
# Frontend is deployed to Vercel separately — not bundled here.
#
# Usage:
#   docker build -t darkreach .
#   docker run -e DATABASE_URL=postgres://... -p 7001:7001 darkreach
#
# Build args:
#   RUST_TARGET_CPU  - CPU target for RUSTFLAGS (default: x86-64-v3 for AVX2)
#
# Volunteer mode:
#   docker run -e API_KEY=ph_xxx -e SERVER=https://darkreach.example.com \
#     ghcr.io/oddurs/darkreach volunteer

# ── Stage 1: Rust build ─────────────────────────────────────────
FROM rust:1-bookworm AS rust-build
WORKDIR /app

# AVX2 (x86-64-v3) for modern servers; override for older or ARM targets
ARG RUST_TARGET_CPU=x86-64-v3
ENV RUSTFLAGS="-C target-cpu=${RUST_TARGET_CPU}"

RUN apt-get update && apt-get install -y --no-install-recommends \
    libgmp-dev m4 pkg-config \
    && rm -rf /var/lib/apt/lists/*

# Cache dependencies by building with a dummy main first
COPY Cargo.toml Cargo.lock ./
COPY gwnum-sys/ gwnum-sys/
RUN mkdir src && echo 'fn main() {}' > src/main.rs \
    && echo 'pub fn dummy() {}' > src/lib.rs \
    && cargo build --release 2>/dev/null || true \
    && rm -rf src

COPY src/ src/
# Touch main.rs so cargo rebuilds it (not the cached dummy)
RUN touch src/main.rs src/lib.rs \
    && cargo build --release

# ── Stage 2: Runtime ─────────────────────────────────────────────
FROM debian:bookworm-slim AS runtime
WORKDIR /app

# OCI metadata labels
LABEL org.opencontainers.image.title="darkreach" \
      org.opencontainers.image.description="Volunteer computing platform for hunting special-form prime numbers" \
      org.opencontainers.image.source="https://github.com/oddurs/darkreach" \
      org.opencontainers.image.licenses="MIT"

RUN apt-get update && apt-get install -y --no-install-recommends \
    libgmp10 ca-certificates \
    && rm -rf /var/lib/apt/lists/*

COPY --from=rust-build /app/target/release/darkreach /usr/local/bin/darkreach

EXPOSE 7001

# Health check for container orchestration (K8s uses its own probes instead)
HEALTHCHECK --interval=30s --timeout=5s --start-period=10s --retries=3 \
  CMD ["darkreach", "--help"]

ENTRYPOINT ["darkreach"]
CMD ["dashboard", "--port", "7001"]
