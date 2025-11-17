FROM rust:1.90.0-bullseye AS base

ARG CARGO_CHEF_VERSION=0.1.73
ARG SCCACHE_VERSION=0.12.0

RUN apt-get update && apt-get -y install ca-certificates libssl-dev protobuf-compiler libprotobuf-dev && \
    cargo install cargo-chef --version ${CARGO_CHEF_VERSION} --locked && \
    cargo install sccache --version ${SCCACHE_VERSION} --locked && \
    cargo install sqlx-cli@0.8.6 --no-default-features --features native-tls,postgres

ENV RUSTC_WRAPPER=/usr/local/cargo/bin/sccache
ENV SCCACHE_DIR=/cache/sccache


FROM base AS planner
WORKDIR /src

COPY . .

RUN cargo chef prepare --recipe-path recipe.json


FROM base AS builder

WORKDIR /src
COPY --from=planner /src/recipe.json recipe.json

RUN cargo chef cook --release --recipe-path recipe.json

COPY . .

RUN cargo install --path gateway/apps/main


FROM debian:bullseye
WORKDIR /app

COPY ./gateway/crates/local_db_store/migrations/ /migrations
COPY ./infrastructure/configurations /configuration

RUN apt-get update && apt-get -y install ca-certificates && \
    mkdir -p /app/infrastructure && \
    ln -s /configuration /app/infrastructure/configurations

COPY --from=builder /usr/local/cargo/bin/gateway_main /usr/local/bin/gateway_main
COPY --from=builder /usr/local/cargo/bin/sqlx /usr/local/bin/sqlx
CMD ["gateway_main"]
