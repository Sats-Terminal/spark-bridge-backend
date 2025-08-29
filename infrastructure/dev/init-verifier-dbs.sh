#!/bin/sh
set -e

psql -v ON_ERROR_STOP=1 --username "$POSTGRES_USER" --dbname "$POSTGRES_DB" <<-EOSQL
    CREATE DATABASE verifier1;
    CREATE DATABASE verifier2;
    CREATE DATABASE verifier3;
    
    -- Create verifier schema in each database
    \c verifier1;
    CREATE SCHEMA IF NOT EXISTS verifier;
    GRANT ALL ON SCHEMA verifier TO postgres;
    
    \c verifier2;
    CREATE SCHEMA IF NOT EXISTS verifier;
    GRANT ALL ON SCHEMA verifier TO postgres;
    
    \c verifier3;
    CREATE SCHEMA IF NOT EXISTS verifier;
    GRANT ALL ON SCHEMA verifier TO postgres;
EOSQL
