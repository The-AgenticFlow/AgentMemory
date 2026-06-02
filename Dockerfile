FROM rust:bookworm AS builder

WORKDIR /app

COPY Cargo.toml Cargo.lock ./
COPY crates ./crates
COPY web ./web

RUN cargo build --release -p engram-server

FROM debian:bookworm-slim AS runtime

ENV ENGRAM_SERVER_ADDR=0.0.0.0:3000
ENV ENGRAM_DATA_DIR=/data

WORKDIR /app

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/* \
    && mkdir -p /data

COPY --from=builder /app/target/release/engram-server /usr/local/bin/engram-server
COPY --from=builder /app/web ./web

RUN useradd --create-home --uid 10001 engram \
    && chown -R engram:engram /app /data

USER engram

EXPOSE 3000

CMD ["engram-server"]
