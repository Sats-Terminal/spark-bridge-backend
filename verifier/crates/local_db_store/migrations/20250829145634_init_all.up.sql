BEGIN TRANSACTION;

CREATE SCHEMA verifier;

CREATE TABLE IF NOT EXISTS verifier.musig_identifier
(
    public_key TEXT    NOT NULL,
    rune_id    TEXT    NOT NULL,
    is_issuer  BOOLEAN NOT NULL,
    dkg_state  JSON    NOT NULL,
    PRIMARY KEY (public_key, rune_id)
);

CREATE TABLE IF NOT EXISTS verifier.sign_session
(
    public_key   TEXT  NOT NULL,
    rune_id      TEXT  NOT NULL,
    session_id   TEXT  NOT NULL,
    tweak        BYTEA NOT NULL,
    message_hash BYTEA NOT NULL,
    metadata     JSON  NOT NULL,
    sign_state   JSON  NOT NULL,
    PRIMARY KEY (session_id),
    FOREIGN KEY (public_key, rune_id) REFERENCES verifier.musig_identifier (public_key, rune_id)
);

CREATE TYPE STATUS_TRANSFERRING AS ENUM ('created', 'processing', 'received');

CREATE TABLE IF NOT EXISTS verifier.tx_ids_statuses
(
    tx_id                 TEXT                NOT NULL,
    gateway_loopback_addr TEXT                NOT NULL,
    tx_response_state     STATUS_TRANSFERRING NOT NULL,
    PRIMARY KEY (tx_id)
);

COMMIT;
