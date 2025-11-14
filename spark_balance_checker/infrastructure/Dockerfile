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

RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=$SCCACHE_DIR,sharing=locked \
    cargo chef prepare --recipe-path recipe.json


FROM base AS builder

WORKDIR /src
COPY --from=planner /src/recipe.json recipe.json

RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=$SCCACHE_DIR,sharing=locked \
    cargo chef cook --release --recipe-path recipe.json

COPY . .

RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=$SCCACHE_DIR,sharing=locked \
    cargo install --path spark_balance_checker/apps/main


# The final base image
FROM debian:bullseye

RUN apt-get update && apt-get -y install ca-certificates

COPY ./infrastructure/configurations /configuration
COPY --from=builder /usr/local/cargo/bin/spark_balance_checker_main /usr/local/bin/spark_balance_checker_main
CMD ["spark_balance_checker_main"]
