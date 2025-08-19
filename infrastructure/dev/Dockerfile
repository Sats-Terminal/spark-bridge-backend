FROM rust:1.88.0 AS builder
WORKDIR /usr/src/spark_helper
COPY . .
RUN cargo install --path ./server_helper

FROM debian:bookworm
RUN apt-get update && \
    apt-get install -y openssh-client && \
    apt-get install -y gettext && \
    rm -rf /var/lib/apt/lists/*

RUN mkdir -p /root/.ssh && \
    chmod 700 /root/.ssh && \
    touch /root/.ssh/known_hosts && \
    chmod 644 /root/.ssh/known_hosts

COPY --from=builder /usr/local/cargo/bin/spark-helper /usr/local/bin/spark-helper
COPY ./configuration/base.toml /configuration/base.toml
COPY ./assets /assets
#COPY ./assets/spark_test_execution /root/.ssh/spark_test_execution
#COPY ./assets/spark_test_execution.pub /root/.ssh/spark_test_execution.pub
#RUN chmod 600 /root/.ssh/spark_test_execution && chmod 644 /root/.ssh/spark_test_execution.pub
RUN chmod 600 ./assets/spark_test_execution && chmod 644 ./assets/spark_test_execution.pub

CMD ["spark-helper"]
