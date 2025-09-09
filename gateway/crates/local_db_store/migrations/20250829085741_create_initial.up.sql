BEGIN TRANSACTION;

CREATE TABLE IF NOT EXISTS musig_identifier
(
    public_key TEXT NOT NULL,
    rune_id TEXT NOT NULL,
    is_issuer BOOLEAN NOT NULL,
    dkg_state JSON NOT NULL,
    PRIMARY KEY (public_key, rune_id)
);

CREATE TABLE IF NOT EXISTS sign_session
(
    session_id TEXT NOT NULL,
    public_key TEXT NOT NULL,
    rune_id TEXT NOT NULL,
    tweak BYTEA NOT NULL,
    message_hash BYTEA NOT NULL,
    metadata JSON NOT NULL,
    sign_state JSON NOT NULL,
    PRIMARY KEY (session_id),
    FOREIGN KEY (public_key, rune_id) REFERENCES musig_identifier(public_key, rune_id)
);

CREATE TABLE IF NOT EXISTS deposit_address
(
    nonce_tweak BYTEA NOT NULL,
    public_key TEXT NOT NULL,
    rune_id TEXT NOT NULL,
    address TEXT NOT NULL,
    is_btc BOOLEAN NOT NULL,
    amount INTEGER NOT NULL,
    confirmation_status JSON NOT NULL,
    PRIMARY KEY (nonce_tweak),
    FOREIGN KEY (public_key, rune_id) REFERENCES musig_identifier(public_key, rune_id)
);

COMMIT;
