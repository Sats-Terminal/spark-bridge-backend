#! /bin/bash

# You should run this script from the root of the project

set -e

GATEWAY_DATABASE_URL="postgres://postgres:postgres@localhost:5470/gateway"
VERIFIER_1_DATABASE_URL="postgres://postgres:postgres@localhost:5471/verifier"
VERIFIER_2_DATABASE_URL="postgres://postgres:postgres@localhost:5472/verifier"
VERIFIER_3_DATABASE_URL="postgres://postgres:postgres@localhost:5473/verifier"
BTC_INDEXER_DATABASE_URL="postgres://postgres:postgres@localhost:5474/btc_indexer"

GATEWAY_MIGRATION_PATH="./gateway/crates/local_db_store/migrations"
VERIFIER_MIGRATION_PATH="./verifier/crates/local_db_store/migrations"
BTC_INDEXER_MIGRATION_PATH="./btc_indexer/crates/local_db_store/migrations"

GATEWAY_CONFIG_PATH="./infrastructure/configurations/gateway/dev.toml"
VERIFIER_1_CONFIG_PATH="./infrastructure/configurations/verifier_1/dev.toml"
VERIFIER_2_CONFIG_PATH="./infrastructure/configurations/verifier_2/dev.toml"
VERIFIER_3_CONFIG_PATH="./infrastructure/configurations/verifier_3/dev.toml"
BTC_INDEXER_CONFIG_PATH="./infrastructure/configurations/btc_indexer/dev.toml"
SPARK_BALANCE_CHECKER_CONFIG_PATH="./infrastructure/configurations/spark_balance_checker/dev.toml"

GATEWAY_LOG_PATH="./logs/gateway.log"
VERIFIER_1_LOG_PATH="./logs/verifier_1.log"
VERIFIER_2_LOG_PATH="./logs/verifier_2.log"
VERIFIER_3_LOG_PATH="./logs/verifier_3.log"
SPARK_BALANCE_CHECKER_LOG_PATH="./logs/spark_balance_checker.log"
BTC_INDEXER_LOG_PATH="./logs/btc_indexer.log"

SPARK_BALANCE_CHECKER_CA_PEM_PATH="./infrastructure/configurations/spark_balance_checker/ca.pem"

# Function to run docker compose and wait for initialization
run_docker_compose_with_wait() {
    echo "Starting docker compose with file: $compose_file"
    docker compose -f "./infrastructure/databases.docker-compose.yml" up -d
    docker compose -f "./infrastructure/bitcoind.docker-compose.yml" up -d
    echo "Initialization wait complete."
}

migrate_databases() {
    echo "Migrating databases..."
    sqlx migrate run --database-url $GATEWAY_DATABASE_URL --source $GATEWAY_MIGRATION_PATH
    sqlx migrate run --database-url $VERIFIER_1_DATABASE_URL --source $VERIFIER_MIGRATION_PATH
    sqlx migrate run --database-url $VERIFIER_2_DATABASE_URL --source $VERIFIER_MIGRATION_PATH
    sqlx migrate run --database-url $VERIFIER_3_DATABASE_URL --source $VERIFIER_MIGRATION_PATH
    sqlx migrate run --database-url $BTC_INDEXER_DATABASE_URL --source $BTC_INDEXER_MIGRATION_PATH
    echo "Databases migrated successfully."
}

build_services() {
    cargo build --bin gateway_main
    cargo build --bin verifier_main
    cargo build --bin btc_indexer_main
    cargo build --bin spark_balance_checker_main
}

run_services() {
    echo "Running services..."

    CONFIG_PATH=$GATEWAY_CONFIG_PATH pm2 start ./target/debug/gateway_main \
        --name gateway \
        --log $GATEWAY_LOG_PATH 
    
    CONFIG_PATH=$VERIFIER_1_CONFIG_PATH pm2 start ./target/debug/verifier_main \
        --name verifier_1 \
        --log $VERIFIER_1_LOG_PATH 
    
    CONFIG_PATH=$VERIFIER_2_CONFIG_PATH pm2 start ./target/debug/verifier_main \
        --name verifier_2 \
        --log $VERIFIER_2_LOG_PATH 
    
    CONFIG_PATH=$VERIFIER_3_CONFIG_PATH pm2 start ./target/debug/verifier_main \
        --name verifier_3 \
        --log $VERIFIER_3_LOG_PATH 

    CONFIG_PATH=$SPARK_BALANCE_CHECKER_CONFIG_PATH pm2 start ./target/debug/spark_balance_checker_main \
        --name spark_balance_checker \
        --log $SPARK_BALANCE_CHECKER_LOG_PATH 

    CONFIG_PATH=$BTC_INDEXER_CONFIG_PATH \
    DATABASE_URL=postgres://postgres:postgres@localhost:5474/btc_indexer \
      pm2 start ./target/debug/btc_indexer_main \
        --name btc_indexer \
        --log $BTC_INDEXER_LOG_PATH 
}

main() {
    run_docker_compose_with_wait
    migrate_databases
    build_services
    run_services
}


main
