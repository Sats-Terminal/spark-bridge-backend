FROM rust:1.89.0-bullseye AS builder

# Create a workspace directory
WORKDIR /usr/src/runes-spark-bridge

# Copy the entire workspace
COPY ./ ./

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

RUN cargo install --path ./spark_balance_checker/apps/main

# The final base image
FROM debian:bullseye

RUN apt-get update

# Copying configs
COPY ./spark_balance_checker/infrastructure/configuration/base.toml /configuration_spark_balance_checker/base.toml

# Copying binaries
COPY --from=builder /usr/local/cargo/bin/spark_balance_checker_main /usr/local/bin/spark_balance_checker_main
CMD ["spark_balance_checker_main"]
#CMD ["tail", "-f", "/dev/null"]
