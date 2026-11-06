# syntax=docker/dockerfile:1

# ---------- frontend ----------
FROM node:22-bookworm-slim AS frontend
WORKDIR /app/frontend
COPY frontend/package.json frontend/package-lock.json ./
RUN npm ci
COPY frontend/ ./
RUN npm run build

# ---------- backend (cargo-chef caches dependency builds) ----------
FROM rust:1.95-bookworm AS chef
RUN cargo install cargo-chef --locked --version 0.1.78
WORKDIR /app

FROM chef AS planner
COPY backend/ ./
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder
COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release --locked --recipe-path recipe.json
COPY backend/ ./
# LICENSE.md is referenced by Cargo.toml (license-file).
COPY LICENSE.md /LICENSE.md
RUN cargo build --release --locked --bin autokarta

# ---------- runtime ----------
FROM debian:bookworm-slim AS runtime
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates tzdata \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --system --uid 10001 --no-create-home autokarta
WORKDIR /app
COPY --from=builder /app/target/release/autokarta /usr/local/bin/autokarta
COPY --from=frontend /app/frontend/build /app/static
COPY LICENSE.md /app/LICENSE.md

ENV BIND_ADDR=0.0.0.0:8080 \
    STATIC_DIR=/app/static \
    COOKIE_SECURE=true \
    RUST_LOG=autokarta=info,tower_http=info
USER autokarta
EXPOSE 8080
CMD ["autokarta", "serve"]
