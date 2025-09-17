

down_docker_compose() {
    echo "Shutting down docker compose..."
    docker compose -f "./infrastructure/databases.docker-compose.yml" down -v
    echo "Docker compose shut down successfully."
}

GATEWAY_DATABASE_URL="postgres://postgres:postgres@localhost:5470/gateway"
VERIFIER_1_DATABASE_URL="postgres://postgres:postgres@localhost:5471/verifier"
VERIFIER_2_DATABASE_URL="postgres://postgres:postgres@localhost:5472/verifier"
VERIFIER_3_DATABASE_URL="postgres://postgres:postgres@localhost:5473/verifier"
BTC_INDEXER_DATABASE_URL="postgres://postgres:postgres@localhost:5474/btc_indexer"

revert_migrations() {
    echo "Reverting migrations..."
    sqlx migrate revert --database-url $GATEWAY_DATABASE_URL --source ./gateway/crates/local_db_store/migrations
    sqlx migrate revert --database-url $VERIFIER_1_DATABASE_URL --source ./verifier/crates/local_db_store/migrations
    sqlx migrate revert --database-url $VERIFIER_2_DATABASE_URL --source ./verifier/crates/local_db_store/migrations
    sqlx migrate revert --database-url $VERIFIER_3_DATABASE_URL --source ./verifier/crates/local_db_store/migrations
    sqlx migrate revert --database-url $BTC_INDEXER_DATABASE_URL --source ./btc_indexer/crates/local_db_store/migrations
    echo "Migrations reverted successfully."
}

shut_down_services() {
    echo "Shutting down services..."
}

main() {
    shut_down_services
    revert_migrations
    down_docker_compose
}

main