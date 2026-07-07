# syntax=docker/dockerfile:1

FROM rust:1.95-slim-bookworm AS builder

WORKDIR /app

COPY Cargo.toml Cargo.lock ./
COPY crates ./crates

RUN cargo build --release -p codegraph-server

FROM debian:bookworm-slim AS runtime

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --create-home --uid 10001 --shell /usr/sbin/nologin codegraph \
    && mkdir -p /workspace /cache \
    && chown -R codegraph:codegraph /workspace /cache

COPY --from=builder /app/target/release/codegraph-server /usr/local/bin/codegraph-server

USER codegraph
WORKDIR /workspace

EXPOSE 3765
VOLUME ["/workspace", "/cache"]

ENTRYPOINT ["/usr/local/bin/codegraph-server"]
CMD ["--host", "0.0.0.0", "--root", "/workspace", "--cache-dir", "/cache"]
