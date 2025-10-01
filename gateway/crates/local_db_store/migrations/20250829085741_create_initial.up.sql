BEGIN TRANSACTION;

CREATE SCHEMA IF NOT EXISTS gateway;

----------- USER IDENTIFIERS -----------

-- Dkg pregenerated shares
CREATE TABLE IF NOT EXISTS gateway.dkg_share
(
    dkg_share_id         UUID PRIMARY KEY NOT NULL DEFAULT gen_random_uuid(),
    dkg_aggregator_state JSONB            NOT NULL
);

CREATE TABLE IF NOT EXISTS gateway.user_identifier
(
    user_uuid    UUID    NOT NULL DEFAULT gen_random_uuid(),
    dkg_share_id UUID    NOT NULL,
    public_key   TEXT    NOT NULL,
    rune_id      TEXT    NOT NULL,
    is_issuer    BOOLEAN NOT NULL,
    PRIMARY KEY (user_uuid, rune_id),
    FOREIGN KEY (dkg_share_id) REFERENCES gateway.dkg_share (dkg_share_id)
);

----------- SIGN_SESSION -----------

CREATE TABLE IF NOT EXISTS gateway.sign_session
(
    session_id          TEXT  NOT NULL,
    dkg_share_id        UUID  NOT NULL,
    tweak               BYTEA,
    message_hash        BYTEA NOT NULL,
    aggregator_metadata JSON  NOT NULL,
    sign_state          JSON  NOT NULL,
    PRIMARY KEY (session_id),
    FOREIGN KEY (dkg_share_id) REFERENCES gateway.dkg_share (dkg_share_id)
);

------------ DEPOSIT_ADDRESS -----------

CREATE TABLE IF NOT EXISTS gateway.deposit_address
(
    nonce_tweak         BYTEA   NOT NULL,
    user_uuid           UUID    NOT NULL,
    rune_id             TEXT    NOT NULL,
    deposit_address     TEXT    NOT NULL,
    bridge_address      TEXT,
    is_btc              BOOLEAN NOT NULL,
    amount              BIGINT  NOT NULL,
    confirmation_status JSON    NOT NULL,
    PRIMARY KEY (user_uuid, nonce_tweak),
    FOREIGN KEY (user_uuid, rune_id) REFERENCES gateway.user_identifier (user_uuid, rune_id)
);

------------ UTXO -----------

CREATE TYPE UTXO_STATUS AS ENUM (
    'pending',
    'confirmed',
    'spent'
    );

CREATE TABLE IF NOT EXISTS gateway.utxo
(
    out_point       TEXT PRIMARY KEY,
    rune_amount     BIGINT      NOT NULL,
    rune_id         TEXT        NOT NULL,
    sats_fee_amount BIGINT,
    status          UTXO_STATUS NOT NULL DEFAULT 'pending',
    btc_address     TEXT        NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_utxo_status ON gateway.utxo (status);

------------ SESSION_REQUESTS -----------

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
    session_id     UUID PRIMARY KEY,
    request_type   REQ_TYPE       NOT NULL,
    request_status REQUEST_STATUS NOT NULL
);

------------ PAYING_UTXO -----------

CREATE TABLE IF NOT EXISTS gateway.paying_utxo
(
    txid                          TEXT   NOT NULL,
    vout                          INT    NOT NULL,
    spark_deposit_address         TEXT   NOT NULL,
    sats_amount                   BIGINT NOT NULL,
    none_anyone_can_pay_signature TEXT   NOT NULL,
    PRIMARY KEY (txid, vout)
);

-- Indexes

CREATE INDEX IF NOT EXISTS dkg_share_index
    ON gateway.dkg_share (dkg_share_id)
    INCLUDE (dkg_aggregator_state);
CREATE INDEX IF NOT EXISTS dkg_share_2_index
    ON gateway.dkg_share (dkg_aggregator_state, dkg_share_id);

CREATE INDEX IF NOT EXISTS user_identifier_index
    ON gateway.user_identifier (dkg_share_id)
    INCLUDE (user_uuid, public_key, rune_id, is_issuer);
CREATE INDEX IF NOT EXISTS user_identifier_2_index
    ON gateway.user_identifier (user_uuid, rune_id)
    INCLUDE (dkg_share_id, public_key, is_issuer);

CREATE INDEX IF NOT EXISTS sign_session_index
    ON gateway.sign_session (session_id)
    INCLUDE (dkg_share_id, tweak, message_hash, aggregator_metadata, sign_state);

CREATE INDEX IF NOT EXISTS deposit_address_index
    ON gateway.deposit_address (user_uuid, rune_id, nonce_tweak)
    INCLUDE (deposit_address, bridge_address, is_btc, amount, confirmation_status);
CREATE INDEX IF NOT EXISTS deposit_address_2_index
    ON gateway.deposit_address (deposit_address)
    INCLUDE (user_uuid, rune_id, nonce_tweak, bridge_address, is_btc, amount, confirmation_status);

COMMIT;

