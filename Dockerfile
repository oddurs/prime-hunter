# Multi-stage Docker build for primehunt
#
# Stage 1: Build the Next.js static frontend
# Stage 2: Build the Rust release binary (with GMP)
# Stage 3: Minimal runtime image with binary + static assets
#
# Usage:
#   docker build -t primehunt .
#   docker run -e DATABASE_URL=postgres://... -p 8080:8080 primehunt

# ── Stage 1: Frontend build ──────────────────────────────────────
FROM node:22-slim AS frontend-build
WORKDIR /app/frontend

COPY frontend/package.json frontend/package-lock.json ./
RUN npm ci --ignore-scripts

COPY frontend/ ./
RUN npm run build

# ── Stage 2: Rust build ─────────────────────────────────────────
FROM rust:1-bookworm AS rust-build
WORKDIR /app

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

# ── Stage 3: Runtime ─────────────────────────────────────────────
FROM debian:bookworm-slim AS runtime
WORKDIR /app

RUN apt-get update && apt-get install -y --no-install-recommends \
    libgmp10 ca-certificates \
    && rm -rf /var/lib/apt/lists/*

COPY --from=rust-build /app/target/release/primehunt /usr/local/bin/primehunt
COPY --from=frontend-build /app/frontend/out/ /app/frontend/out/

EXPOSE 8080

ENTRYPOINT ["primehunt"]
CMD ["dashboard", "--port", "8080", "--static-dir", "/app/frontend/out"]
