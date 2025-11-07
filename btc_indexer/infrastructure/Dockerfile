FROM rust:1.90.0-bullseye AS builder
WORKDIR /usr/src/runes-spark-bridge

RUN apt-get update && apt-get -y install ca-certificates libssl-dev protobuf-compiler
RUN cargo install sqlx-cli@0.8.6 --no-default-features --features native-tls,postgres

COPY ./ ./
RUN cargo install --path ./btc_indexer/apps/main

FROM debian:bullseye

# Copying configs
COPY ./btc_indexer/crates/local_db_store/migrations/ /migrations

COPY --from=builder /usr/local/cargo/bin/btc_indexer_main /usr/local/bin/btc_indexer
COPY --from=builder /usr/local/cargo/bin/sqlx /usr/local/bin/sqlx
CMD ["btc_indexer"]
