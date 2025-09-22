FROM rust:1.90.0 AS builder
WORKDIR /usr/src/runes-spark-bridge
COPY ./ ./
RUN cargo install --path ./btc_indexer/apps/main

FROM debian:bookworm

RUN apt-get update && apt-get install -y libssl-dev

COPY --from=builder /usr/local/cargo/bin/btc_indexer_main /usr/local/bin/btc_indexer
CMD ["btc_indexer"]
#CMD ["tail", "-f", "/dev/null"]
