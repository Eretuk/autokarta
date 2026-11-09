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
# pg_dump for the backup service (same image, see docker-compose.yml). Its
# major version must be at least the server's, so it comes from the
# PostgreSQL apt repository, matching postgres:18 in the compose files.
ARG PG_MAJOR=18
RUN set -eux; \
    apt-get update; \
    apt-get install -y --no-install-recommends ca-certificates curl tzdata; \
    install -d /usr/share/postgresql-common/pgdg; \
    curl -fsSL -o /usr/share/postgresql-common/pgdg/apt.postgresql.org.asc \
        https://www.postgresql.org/media/keys/ACCC4CF8.asc; \
    echo "deb [signed-by=/usr/share/postgresql-common/pgdg/apt.postgresql.org.asc] https://apt.postgresql.org/pub/repos/apt bookworm-pgdg main" \
        > /etc/apt/sources.list.d/pgdg.list; \
    apt-get update; \
    apt-get install -y --no-install-recommends "postgresql-client-${PG_MAJOR}"; \
    apt-get purge -y --auto-remove curl; \
    # Perl (~50 MB) is only for Debian's pg_wrapper; pg_dump itself is a C
    # binary, called directly through PATH below.
    dpkg --purge --force-depends postgresql-client-common perl perl-modules-5.36 libperl5.36; \
    rm -rf /var/lib/apt/lists/*; \
    useradd --system --uid 10001 --no-create-home autokarta
WORKDIR /app
COPY --from=builder /app/target/release/autokarta /usr/local/bin/autokarta
COPY --from=frontend /app/frontend/build /app/static
COPY deploy/backup.sh /usr/local/bin/autokarta-backup
COPY LICENSE.md /app/LICENSE.md

ENV PATH=/usr/lib/postgresql/${PG_MAJOR}/bin:$PATH \
    BIND_ADDR=0.0.0.0:8080 \
    STATIC_DIR=/app/static \
    COOKIE_SECURE=true \
    RUST_LOG=autokarta=info,tower_http=info
USER autokarta
EXPOSE 8080
CMD ["autokarta", "serve"]
