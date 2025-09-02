FROM rust:1.89.0-bullseye AS builder

# Create a workspace directory
WORKDIR /usr/src/runes-spark-bridge

# Copy the entire workspace
COPY ./ ./
RUN cargo install --path ./gateway/apps/main
RUN cargo install sqlx-cli@0.8.6 --no-default-features --features native-tls,postgres


# The final base image
FROM debian:bullseye

RUN apt-get update && apt-get install -y libssl-dev

# Copying configs
COPY ./gateway/infrastructure/configuration/base.toml /configuration_gateway/base.toml
COPY ./gateway/crates/local_db_store/migrations/ /migrations

# Copying binaries
COPY --from=builder /usr/local/cargo/bin/gateway_main /usr/local/bin/gateway_main
COPY --from=builder /usr/local/cargo/bin/sqlx /usr/local/bin/sqlx
CMD ["verifier_main"]
#CMD ["tail", "-f", "/dev/null"]
