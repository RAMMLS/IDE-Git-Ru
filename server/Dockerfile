FROM rust:1.86-bookworm AS builder

WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY ide ./ide
COPY vcs-core ./vcs-core
COPY server ./server

RUN cargo build --release -p aura-server

FROM debian:bookworm-slim AS runtime

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates curl \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --system --create-home --home-dir /home/aura aura \
    && mkdir -p /data/aura \
    && chown -R aura:aura /data/aura

COPY --from=builder /app/target/release/aura-server /usr/local/bin/aura-server

USER aura
ENV AURA_STORAGE_ROOT=/data/aura
ENV AURA_API_PORT=3000
VOLUME ["/data/aura"]
EXPOSE 3000

ENTRYPOINT ["/usr/local/bin/aura-server"]
