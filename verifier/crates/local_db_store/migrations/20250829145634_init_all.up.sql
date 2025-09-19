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

CREATE TABLE IF NOT EXISTS verifier.deposit_address
(
    nonce_tweak BYTEA NOT NULL,
    public_key TEXT NOT NULL,
    rune_id TEXT NOT NULL,
    deposit_address TEXT NOT NULL,
    bridge_address TEXT NOT NULL,
    is_btc BOOLEAN NOT NULL,
    deposit_amount BIGINT NOT NULL,
    confirmation_status JSON NOT NULL,
    sats_fee_amount BIGINT,
    out_point TEXT,
    PRIMARY KEY (public_key, rune_id, nonce_tweak),
    FOREIGN KEY (public_key, rune_id) REFERENCES verifier.musig_identifier(public_key, rune_id)
);

COMMIT;
