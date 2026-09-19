# syntax=docker/dockerfile:1

FROM rust:1-bookworm AS builder
WORKDIR /app

RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        cmake \
        clang \
        pkg-config \
        libssl-dev \
        perl \
        make \
        g++ \
    && rm -rf /var/lib/apt/lists/*

COPY Cargo.toml Cargo.lock ./
COPY .cargo .cargo
COPY src src
COPY data data

RUN cargo build --release --no-default-features --locked --bin voidreg-api --bin migrate

FROM debian:bookworm-slim
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates libssl3 curl \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --create-home --uid 1000 --user-group app \
    && mkdir -p /data \
    && chown app:app /data

COPY --from=builder /app/target/release/voidreg-api /usr/local/bin/voidreg-api
COPY --from=builder /app/target/release/migrate /usr/local/bin/migrate
COPY --chmod=755 docker/entrypoint.sh /usr/local/bin/entrypoint.sh

USER app
WORKDIR /home/app
EXPOSE 8080
VOLUME /data

ENV HOST=0.0.0.0 \
    PORT=8080 \
    TURSO_LOCAL_PATH=/data/voidreg.db

HEALTHCHECK --interval=10s --timeout=3s --start-period=40s --retries=5 \
    CMD curl -fsS http://127.0.0.1:8080/healthz || exit 1

ENTRYPOINT ["/usr/local/bin/entrypoint.sh"]
