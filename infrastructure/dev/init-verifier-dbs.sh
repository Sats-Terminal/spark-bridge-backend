#!/bin/sh
set -e

psql -v ON_ERROR_STOP=1 --username "$POSTGRES_USER" --dbname "$POSTGRES_DB" <<-EOSQL
    CREATE DATABASE verifier1;
    CREATE DATABASE verifier2;
    CREATE DATABASE verifier3;
EOSQL
