BEGIN TRANSACTION;

-- for debugging purposes
CREATE SCHEMA gateway;

CREATE TABLE IF NOT EXISTS gateway.musig_identifier
(
    public_key TEXT NOT NULL,
    rune_id TEXT NOT NULL,
    is_issuer BOOLEAN NOT NULL,
    dkg_state JSON NOT NULL,
    PRIMARY KEY (public_key, rune_id)
);

CREATE TABLE IF NOT EXISTS gateway.sign_session
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

CREATE TABLE IF NOT EXISTS gateway.deposit_address
(
    nonce_tweak BYTEA NOT NULL,
    public_key TEXT NOT NULL,
    rune_id TEXT NOT NULL,
    address TEXT,
    is_btc BOOLEAN NOT NULL,
    amount BIGINT NOT NULL,
    txid TEXT,
    confirmation_status JSON NOT NULL,
    PRIMARY KEY (public_key, rune_id, nonce_tweak),
    FOREIGN KEY (public_key, rune_id) REFERENCES gateway.musig_identifier(public_key, rune_id)
);

COMMIT;

