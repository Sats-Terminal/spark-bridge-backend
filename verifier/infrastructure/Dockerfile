FROM rust:1.89.0-bullseye AS builder

# Create a workspace directory
WORKDIR /usr/src/runes-spark-bridge

# Installing proto
ENV PROTOC_VERSION=32.0
RUN apt update && apt upgrade -y

# Download proto zip
ENV PROTOC_VERSION=32.0
ENV PROTOC_ZIP=protoc-${PROTOC_VERSION}-linux-x86_64.zip
RUN curl -OL https://github.com/protocolbuffers/protobuf/releases/download/v${PROTOC_VERSION}/${PROTOC_ZIP}
RUN unzip -o ${PROTOC_ZIP} -d ./proto
RUN chmod 755 -R ./proto/bin
ENV BASE=/usr

# Copy into path
RUN cp ./proto/bin/protoc ${BASE}/bin/
RUN cp -R ./proto/include/* ${BASE}/include/

# Copy the entire workspace
COPY ./ ./
RUN cargo install --path ./verifier/apps/main
RUN cargo install sqlx-cli@0.8.6 --no-default-features --features native-tls,postgres


# The final base image
FROM debian:bullseye

# Copying configs
COPY ./verifier/crates/local_db_store/migrations/ /migrations

# Copying binaries
COPY --from=builder /usr/local/cargo/bin/verifier_main /usr/local/bin/verifier_main
COPY --from=builder /usr/local/cargo/bin/sqlx /usr/local/bin/sqlx
CMD ["verifier_main"]
#CMD ["tail", "-f", "/dev/null"]
