#! /bin/bash

set -e

CONFIG_PATH=$1

if [ -z "$CONFIG_PATH" ]; then
    echo "CONFIG_PATH is not set"
    exit 1
fi

CONFIG_PATH=$CONFIG_PATH cargo run --bin spark_balance_checker_main
