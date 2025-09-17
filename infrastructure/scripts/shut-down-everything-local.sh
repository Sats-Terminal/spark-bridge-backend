

down_docker_compose() {
    echo "Shutting down docker compose..."
    docker compose -f "./infrastructure/databases.docker-compose.yml" down -v
    docker compose -f "./infrastructure/bitcoind.docker-compose.yml" down -v
    echo "Docker compose shut down successfully."
}

GATEWAY_DATABASE_URL="postgres://postgres:postgres@localhost:5470/gateway"
VERIFIER_1_DATABASE_URL="postgres://postgres:postgres@localhost:5471/verifier"
VERIFIER_2_DATABASE_URL="postgres://postgres:postgres@localhost:5472/verifier"
VERIFIER_3_DATABASE_URL="postgres://postgres:postgres@localhost:5473/verifier"
BTC_INDEXER_DATABASE_URL="postgres://postgres:postgres@localhost:5474/btc_indexer"

GATEWAY_MIGRATION_PATH="./gateway/crates/local_db_store/migrations"
VERIFIER_MIGRATION_PATH="./verifier/crates/local_db_store/migrations"
BTC_INDEXER_MIGRATION_PATH="./btc_indexer/crates/local_db_store/migrations"

revert_migrations() {
    echo "Reverting migrations..."
    sqlx migrate revert --database-url $GATEWAY_DATABASE_URL --source $GATEWAY_MIGRATION_PATH
    sqlx migrate revert --database-url $VERIFIER_1_DATABASE_URL --source $VERIFIER_MIGRATION_PATH
    sqlx migrate revert --database-url $VERIFIER_2_DATABASE_URL --source $VERIFIER_MIGRATION_PATH
    sqlx migrate revert --database-url $VERIFIER_3_DATABASE_URL --source $VERIFIER_MIGRATION_PATH
    sqlx migrate revert --database-url $BTC_INDEXER_DATABASE_URL --source $BTC_INDEXER_MIGRATION_PATH
    echo "Migrations reverted successfully."
}

shut_down_services() {
    echo "Shutting down services..."
    pm2 delete all
    rm -rf ./logs
}

main() {
    shut_down_services
    revert_migrations
    down_docker_compose
}

main
