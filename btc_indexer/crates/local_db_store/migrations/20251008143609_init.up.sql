-- Add up migration script here

CREATE SCHEMA IF NOT EXISTS btc_indexer;

CREATE TYPE WATCH_REQUEST_STATUS AS ENUM (
    'pending',
    'confirmed',
    'failed'
);

CREATE TABLE IF NOT EXISTS btc_indexer.requests
(
    id UUID PRIMARY KEY,
    request_id UUID NOT NULL,
    outpoint TEXT NOT NULL,
    btc_address TEXT NOT NULL,
    rune_id TEXT,
    rune_amount BIGINT,
    sats_amount BIGINT,
    created_at BIGINT NOT NULL,
    status WATCH_REQUEST_STATUS NOT NULL,
    error_details JSON,
    callback_url TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS btc_indexer.txs
(
    txid BYTEA PRIMARY KEY,
    block_height BIGINT NOT NULL
);
