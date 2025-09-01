FROM rust:1.89.0 AS builder
WORKDIR /usr/src/runes-spark-bridge
COPY ./ ./
RUN cargo install --path ./verifier/apps/main

FROM debian:bookworm

RUN apt-get update && apt-get install -y libssl-dev

COPY --from=builder /usr/local/cargo/bin/verifier_main /usr/local/bin/verifier_main
COPY ./verifier/infrastructure/configuration/base.toml /configuration_verifier/base.toml
CMD ["verifier_main"]
#CMD ["tail", "-f", "/dev/null"]
