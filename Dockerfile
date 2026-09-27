FROM rust:1.98-bookworm AS builder
WORKDIR /app

COPY Cargo.toml Cargo.lock ./
COPY src ./src
RUN cargo build --release

FROM debian:bookworm-slim
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --system --uid 10001 --create-home logbook

WORKDIR /app
COPY --from=builder /app/target/release/logbook /usr/local/bin/logbook

RUN mkdir -p /data && chown logbook:logbook /data

ENV LOGBOOK_DB_PATH=/data/logbook.db
ENV LOGBOOK_PORT=8080
EXPOSE 8080
VOLUME ["/data"]
USER logbook

CMD ["/usr/local/bin/logbook"]
