BEGIN TRANSACTION;

CREATE SCHEMA IF NOT EXISTS verifier;

----------- USER IDENTIFIERS -----------

CREATE TABLE IF NOT EXISTS verifier.dkg_share
(
    dkg_share_id     UUID PRIMARY KEY NOT NULL,
    dkg_signer_state JSONB            NOT NULL
);


----------- SIGN_SESSION -----------

CREATE TABLE IF NOT EXISTS verifier.sign_session
(
    session_id   TEXT  NOT NULL,
    dkg_share_id UUID  NOT NULL,
    tweak        BYTEA,
    message_hash BYTEA NOT NULL,
    metadata     JSON  NOT NULL,
    sign_state   JSON  NOT NULL,
    PRIMARY KEY (session_id),
    FOREIGN KEY (dkg_share_id) REFERENCES verifier.dkg_share (dkg_share_id)
);

----------- USER_IDENTIFIER -----------

CREATE TABLE IF NOT EXISTS verifier.user_identifier
(
    dkg_share_id UUID    NOT NULL,
    user_id    UUID    NOT NULL,
    rune_id      TEXT    NOT NULL,
    is_issuer    BOOLEAN NOT NULL,
    PRIMARY KEY (dkg_share_id),
    FOREIGN KEY (dkg_share_id) REFERENCES verifier.dkg_share (dkg_share_id)
);

------------ DEPOSIT_ADDRESS -----------

CREATE TYPE DEPOSIT_STATUS AS ENUM (
    'pending',
    'confirmed',
    'failed'
);

CREATE TABLE IF NOT EXISTS verifier.deposit_address
(
    nonce_tweak         BYTEA   NOT NULL,
    dkg_share_id        UUID    NOT NULL,
    deposit_address     TEXT    NOT NULL,
    bridge_address      TEXT    NOT NULL,
    is_btc              BOOLEAN NOT NULL,
    deposit_amount      BIGINT  NOT NULL,
    confirmation_status DEPOSIT_STATUS    NOT NULL,
    error_details       TEXT,
    sats_amount     BIGINT,
    outpoint           TEXT,
    PRIMARY KEY (dkg_share_id),
    FOREIGN KEY (dkg_share_id) REFERENCES verifier.user_identifier (dkg_share_id)
);

------------ SESSION -----------

CREATE TYPE REQUEST_TYPE AS ENUM (
    'bridge_runes',
    'exit_spark'
);

CREATE TYPE REQUEST_STATUS AS ENUM (
    'pending',
    'completed',
    'failed'
);

CREATE TABLE IF NOT EXISTS verifier.sessions
(
    request_id     UUID PRIMARY KEY,
    request_type   REQUEST_TYPE   NOT NULL,
    request_status REQUEST_STATUS NOT NULL,
    error_details TEXT
);

COMMIT;
