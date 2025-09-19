BEGIN TRANSACTION;

CREATE SCHEMA gateway;

+---------- MUSIG_IDENTIFIER -----------

CREATE TABLE IF NOT EXISTS gateway.musig_identifier
(
    public_key TEXT NOT NULL,
    rune_id TEXT NOT NULL,
    is_issuer BOOLEAN NOT NULL,
    dkg_state JSON NOT NULL,
    PRIMARY KEY (public_key, rune_id)
);

+---------- SIGN_SESSION -----------

CREATE TABLE IF NOT EXISTS gateway.sign_session
(
    session_id TEXT NOT NULL,
    public_key TEXT NOT NULL,
    rune_id TEXT NOT NULL,
    tweak BYTEA NOT NULL,
    message_hash BYTEA NOT NULL,
    metadata JSON NOT NULL,
    sign_state JSON NOT NULL,
    PRIMARY KEY (session_id),
    FOREIGN KEY (public_key, rune_id) REFERENCES gateway.musig_identifier(public_key, rune_id)
);

+----------- DEPOSIT_ADDRESS -----------

CREATE TABLE IF NOT EXISTS gateway.deposit_address
(
    nonce_tweak BYTEA NOT NULL,
    public_key TEXT NOT NULL,
    rune_id TEXT NOT NULL,
    deposit_address TEXT NOT NULL,
    bridge_address TEXT,
    is_btc BOOLEAN NOT NULL,
    amount BIGINT NOT NULL,
    confirmation_status JSON NOT NULL,
    PRIMARY KEY (public_key, rune_id, nonce_tweak),
    FOREIGN KEY (public_key, rune_id) REFERENCES gateway.musig_identifier(public_key, rune_id)
);

+----------- UTXO -----------

CREATE TYPE UTXO_STATUS AS ENUM (
    'pending',
    'confirmed',
    'spent'
);

CREATE TABLE IF NOT EXISTS gateway.utxo
(
    out_point    TEXT        PRIMARY KEY,
    rune_amount       BIGINT      NOT NULL,
    rune_id      TEXT        NOT NULL,
    sats_fee_amount BIGINT   ,
    status       UTXO_STATUS NOT NULL DEFAULT 'pending',
    btc_address  TEXT        NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_utxo_status ON gateway.utxo (status);

+----------- SESSION_REQUESTS -----------

CREATE TYPE REQ_TYPE AS ENUM (
    'get_runes_deposit_address',
    'get_spark_deposit_address',
    'bridge_runes',
    'exit_spark'
);

CREATE TYPE REQUEST_STATUS AS ENUM (
    'pending',
    'processing',
    'completed',
    'failed',
    'cancelled'
    );

CREATE TABLE IF NOT EXISTS gateway.session_requests
(
    session_id   UUID PRIMARY KEY,
    request_type REQ_TYPE       NOT NULL,
    request_status REQUEST_STATUS NOT NULL
);

+----------- PAYING_UTXO -----------

CREATE TABLE IF NOT EXISTS gateway.paying_utxo
(
    txid TEXT NOT NULL,
    vout INT NOT NULL,
    spark_deposit_address TEXT NOT NULL,
    sats_amount BIGINT NOT NULL,
    none_anyone_can_pay_signature TEXT NOT NULL,
    PRIMARY KEY (txid, vout)
);

COMMIT;

