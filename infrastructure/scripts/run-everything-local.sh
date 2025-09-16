#! /bin/bash

# You should run this script from the root of the project

# Function to run docker compose and wait for initialization
run_docker_compose_with_wait() {
    echo "Starting docker compose with file: $compose_file"
    docker compose -f "./infrastructure/databases.docker-compose.yml" up -d
    
    echo "Docker compose started successfully. Waiting 10 seconds for initialization..."
    sleep 10
    echo "Initialization wait complete."
}

GATEWAY_DATABASE_URL="postgres://postgres:postgres@localhost:5470/gateway"
VERIFIER_1_DATABASE_URL="postgres://postgres:postgres@localhost:5471/verifier"
VERIFIER_2_DATABASE_URL="postgres://postgres:postgres@localhost:5472/verifier"
VERIFIER_3_DATABASE_URL="postgres://postgres:postgres@localhost:5473/verifier"
BTC_INDEXER_DATABASE_URL="postgres://postgres:postgres@localhost:5474/btc_indexer"

migrate_databases() {
    echo "Migrating databases..."
    sqlx migrate run --database-url $GATEWAY_DATABASE_URL --source ./gateway/crates/local_db_store/migrations
    sqlx migrate run --database-url $VERIFIER_1_DATABASE_URL --source ./verifier/crates/local_db_store/migrations
    sqlx migrate run --database-url $VERIFIER_2_DATABASE_URL --source ./verifier/crates/local_db_store/migrations
    sqlx migrate run --database-url $VERIFIER_3_DATABASE_URL --source ./verifier/crates/local_db_store/migrations
    sqlx migrate run --database-url $BTC_INDEXER_DATABASE_URL --source ./btc_indexer/crates/local_db_store/migrations
    echo "Databases migrated successfully."
}

run_services() {
    
}

main() {
    run_docker_compose_with_wait
    migrate_databases
}


main
