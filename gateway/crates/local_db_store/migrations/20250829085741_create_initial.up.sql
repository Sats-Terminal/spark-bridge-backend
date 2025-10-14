BEGIN TRANSACTION;

CREATE SCHEMA IF NOT EXISTS gateway;

----------- DKG SHARE -----------

-- Dkg pregenerated shares
CREATE TABLE IF NOT EXISTS gateway.dkg_share
(
    dkg_share_id         UUID PRIMARY KEY NOT NULL DEFAULT gen_random_uuid(),
    dkg_aggregator_state JSONB            NOT NULL
);

----------- USER IDENTIFIERS -----------

CREATE TABLE IF NOT EXISTS gateway.user_identifier
(
    dkg_share_id UUID    NOT NULL,
    user_id    UUID    NOT NULL,
    rune_id      TEXT    NOT NULL,
    is_issuer    BOOLEAN NOT NULL,
    PRIMARY KEY (dkg_share_id),
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
    dkg_share_id        UUID    NOT NULL,
    deposit_address     TEXT    NOT NULL,
    bridge_address      TEXT,
    is_btc              BOOLEAN NOT NULL,
    amount              BIGINT  NOT NULL,
    confirmation_status JSON    NOT NULL,
    PRIMARY KEY (nonce_tweak),
    FOREIGN KEY (dkg_share_id) REFERENCES gateway.user_identifier (dkg_share_id),
    UNIQUE (deposit_address)
);

------------ UTXO -----------

CREATE TYPE UTXO_STATUS AS ENUM (
    'pending',
    'confirmed',
    'spent'
);

CREATE TABLE IF NOT EXISTS gateway.utxo
(
    outpoint       TEXT PRIMARY KEY,
    rune_amount     BIGINT      NOT NULL,
    rune_id         TEXT        NOT NULL,
    sats_amount BIGINT,
    status          UTXO_STATUS NOT NULL,
    btc_address     TEXT        NOT NULL,
    FOREIGN KEY (btc_address) REFERENCES gateway.deposit_address (deposit_address)
);

CREATE INDEX IF NOT EXISTS idx_utxo_status ON gateway.utxo (status);

------------ SESSION_REQUESTS -----------

CREATE TYPE REQUEST_TYPE AS ENUM (
    'bridge_runes',
    'exit_spark'
);

CREATE TYPE REQUEST_STATUS AS ENUM (
    'pending',
    'completed',
    'failed'
);

CREATE TABLE IF NOT EXISTS gateway.sessions
(
    request_id     UUID PRIMARY KEY,
    request_type   REQUEST_TYPE       NOT NULL,
    request_status REQUEST_STATUS NOT NULL,
    deposit_address TEXT NOT NULL,
    error_details JSON,
    FOREIGN KEY (deposit_address) REFERENCES gateway.deposit_address (deposit_address)
);

------------ PAYING_UTXO -----------

CREATE TABLE IF NOT EXISTS gateway.paying_utxo
(
    txid                          TEXT   NOT NULL,
    vout                          INT    NOT NULL,
    btc_exit_address         TEXT   NOT NULL,
    sats_amount                   BIGINT NOT NULL,
    none_anyone_can_pay_signature TEXT   NOT NULL,
    PRIMARY KEY (txid, vout)
);

-- Indexes

COMMIT;
