#!/usr/bin/env bash

# You should run this script from the root of the project

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT_DIR"

LOG_DIR="${ROOT_DIR}/logs"
mkdir -p "$LOG_DIR"

#######################################
# Helpers
#######################################

load_env_file() {
    local env_file="$1"
    if [[ -f "$env_file" ]]; then
        echo "Loading environment variables from $env_file"
        set -a
        # shellcheck disable=SC1090
        source "$env_file"
        set +a
    fi
}

ensure_command() {
    local cmd="$1"
    if ! command -v "$cmd" >/dev/null 2>&1; then
        echo "Missing required command: $cmd" >&2
        exit 1
    fi
}

#######################################
# Env loading (allows external overrides)
#######################################

load_env_file "./.env"
load_env_file "./.env.mainnet"
load_env_file "./gateway/.env"
load_env_file "./verifier/.env"
load_env_file "./btc_indexer/.env"
load_env_file "./spark_balance_checker/.env"

#######################################
# Configuration (overridable via env)
#######################################

DEFAULT_GATEWAY_DB="postgres://postgres:postgres@localhost:5470/gateway"
DEFAULT_VERIFIER_DB="postgres://postgres:postgres@localhost:5471/verifier"
DEFAULT_VERIFIER2_DB="postgres://postgres:postgres@localhost:5472/verifier"
DEFAULT_VERIFIER3_DB="postgres://postgres:postgres@localhost:5473/verifier"
DEFAULT_BTC_INDEXER_DB="postgres://postgres:postgres@localhost:5474/btc_indexer"

GATEWAY_DATABASE_URL="${GATEWAY_DATABASE_URL:-$DEFAULT_GATEWAY_DB}"
VERIFIER_1_DATABASE_URL="${VERIFIER_1_DATABASE_URL:-$DEFAULT_VERIFIER_DB}"
VERIFIER_2_DATABASE_URL="${VERIFIER_2_DATABASE_URL:-$DEFAULT_VERIFIER2_DB}"
VERIFIER_3_DATABASE_URL="${VERIFIER_3_DATABASE_URL:-$DEFAULT_VERIFIER3_DB}"
BTC_INDEXER_DATABASE_URL="${BTC_INDEXER_DATABASE_URL:-$DEFAULT_BTC_INDEXER_DB}"

DEFAULT_GATEWAY_CONFIG="./infrastructure/configurations/gateway/dev.toml"
DEFAULT_VERIFIER1_CONFIG="./infrastructure/configurations/verifier_1/dev.toml"
DEFAULT_VERIFIER2_CONFIG="./infrastructure/configurations/verifier_2/dev.toml"
DEFAULT_VERIFIER3_CONFIG="./infrastructure/configurations/verifier_3/dev.toml"
DEFAULT_BTC_INDEXER_CONFIG="./infrastructure/configurations/btc_indexer/dev.toml"
DEFAULT_SPARK_BALANCE_CONFIG="./infrastructure/configurations/spark_balance_checker/dev.toml"

GATEWAY_CONFIG_PATH="${GATEWAY_CONFIG_PATH:-$DEFAULT_GATEWAY_CONFIG}"
VERIFIER_1_CONFIG_PATH="${VERIFIER_1_CONFIG_PATH:-$DEFAULT_VERIFIER1_CONFIG}"
VERIFIER_2_CONFIG_PATH="${VERIFIER_2_CONFIG_PATH:-$DEFAULT_VERIFIER2_CONFIG}"
VERIFIER_3_CONFIG_PATH="${VERIFIER_3_CONFIG_PATH:-$DEFAULT_VERIFIER3_CONFIG}"
BTC_INDEXER_CONFIG_PATH="${BTC_INDEXER_CONFIG_PATH:-$DEFAULT_BTC_INDEXER_CONFIG}"
SPARK_BALANCE_CHECKER_CONFIG_PATH="${SPARK_BALANCE_CHECKER_CONFIG_PATH:-$DEFAULT_SPARK_BALANCE_CONFIG}"

GATEWAY_LOG_PATH="${GATEWAY_LOG_PATH:-${LOG_DIR}/gateway.log}"
VERIFIER_1_LOG_PATH="${VERIFIER_1_LOG_PATH:-${LOG_DIR}/verifier_1.log}"
VERIFIER_2_LOG_PATH="${VERIFIER_2_LOG_PATH:-${LOG_DIR}/verifier_2.log}"
VERIFIER_3_LOG_PATH="${VERIFIER_3_LOG_PATH:-${LOG_DIR}/verifier_3.log}"
SPARK_BALANCE_CHECKER_LOG_PATH="${SPARK_BALANCE_CHECKER_LOG_PATH:-${LOG_DIR}/spark_balance_checker.log}"
BTC_INDEXER_LOG_PATH="${BTC_INDEXER_LOG_PATH:-${LOG_DIR}/btc_indexer.log}"

GATEWAY_MIGRATION_PATH="./gateway/crates/local_db_store/migrations"
VERIFIER_MIGRATION_PATH="./verifier/crates/local_db_store/migrations"
BTC_INDEXER_MIGRATION_PATH="./btc_indexer/crates/local_db_store/migrations"

START_LOCAL_INFRA="${START_LOCAL_INFRA:-0}"   # set to 1 if you still want regtest docker infra
RUN_MIGRATIONS="${RUN_MIGRATIONS:-1}"
BUILD_PROFILE="${BUILD_PROFILE:-debug}"       # set to release for optimized binaries

#######################################
# Tasks
#######################################

run_local_infra() {
    if [[ "$START_LOCAL_INFRA" == "1" ]]; then
        echo "Starting local docker infrastructure (regtest/Titan)."
        docker compose -f "./infrastructure/databases.docker-compose.yml" up -d
        docker compose -f "./infrastructure/bitcoind.docker-compose.yml" up -d
    else
        echo "Skipping docker compose startup (assuming external mainnet services are available)."
    fi
}

migrate_databases() {
    if [[ "$RUN_MIGRATIONS" != "1" ]]; then
        echo "Skipping migrations (RUN_MIGRATIONS != 1)."
        return
    fi

    echo "Running database migrations..."
    sqlx migrate run --database-url "$GATEWAY_DATABASE_URL" --source "$GATEWAY_MIGRATION_PATH"
    sqlx migrate run --database-url "$VERIFIER_1_DATABASE_URL" --source "$VERIFIER_MIGRATION_PATH"
    sqlx migrate run --database-url "$VERIFIER_2_DATABASE_URL" --source "$VERIFIER_MIGRATION_PATH"
    sqlx migrate run --database-url "$VERIFIER_3_DATABASE_URL" --source "$VERIFIER_MIGRATION_PATH"
    sqlx migrate run --database-url "$BTC_INDEXER_DATABASE_URL" --source "$BTC_INDEXER_MIGRATION_PATH"
    echo "Migrations completed."
}

build_services() {
    local profile_flag=""
    if [[ "$BUILD_PROFILE" == "release" ]]; then
        profile_flag="--release"
    fi

    echo "Building binaries ($BUILD_PROFILE)..."
    cargo build $profile_flag --bin gateway_main
    cargo build $profile_flag --bin verifier_main
    cargo build $profile_flag --bin btc_indexer_main
    cargo build $profile_flag --bin spark_balance_checker_main
}

pm2_bin_path() {
    if command -v pm2 >/dev/null 2>&1; then
        echo "pm2"
    elif command -v npx >/dev/null 2>&1; then
        echo "npx pm2"
    else
        echo ""
    fi
}

start_service() {
    local name="$1"
    local binary="$2"
    local config_path="$3"
    local log_path="$4"
    local profile_dir="target/$BUILD_PROFILE"
    local pm2_cmd
    pm2_cmd="$(pm2_bin_path)"

    if [[ -z "$pm2_cmd" ]]; then
        echo "pm2 is required to manage processes. Please install pm2."
        exit 1
    fi

    echo "Starting $name with config $config_path"
    CONFIG_PATH="$config_path" $pm2_cmd start "./$profile_dir/$binary" \
        --name "$name" \
        --log "$log_path" \
        --time
}

run_services() {
    start_service "gateway" "gateway_main" "$GATEWAY_CONFIG_PATH" "$GATEWAY_LOG_PATH"
    start_service "verifier_1" "verifier_main" "$VERIFIER_1_CONFIG_PATH" "$VERIFIER_1_LOG_PATH"
    start_service "verifier_2" "verifier_main" "$VERIFIER_2_CONFIG_PATH" "$VERIFIER_2_LOG_PATH"
    start_service "verifier_3" "verifier_main" "$VERIFIER_3_CONFIG_PATH" "$VERIFIER_3_LOG_PATH"
    start_service "spark_balance_checker" "spark_balance_checker_main" "$SPARK_BALANCE_CHECKER_CONFIG_PATH" "$SPARK_BALANCE_CHECKER_LOG_PATH"
    start_service "btc_indexer" "btc_indexer_main" "$BTC_INDEXER_CONFIG_PATH" "$BTC_INDEXER_LOG_PATH"
}

#######################################
# Main
#######################################

main() {
    ensure_command sqlx
    ensure_command cargo

    run_local_infra
    migrate_databases
    build_services
    run_services

    echo "All services launched with configuration:"
    echo "  Gateway config:            $GATEWAY_CONFIG_PATH"
    echo "  Verifier configs:          $VERIFIER_1_CONFIG_PATH, $VERIFIER_2_CONFIG_PATH, $VERIFIER_3_CONFIG_PATH"
    echo "  BTC Indexer config:        $BTC_INDEXER_CONFIG_PATH"
    echo "  Spark balance checker:     $SPARK_BALANCE_CHECKER_CONFIG_PATH"
    echo "  Logs directory:            $LOG_DIR"
}

main "$@"
