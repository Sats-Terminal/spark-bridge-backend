BEGIN TRANSACTION;

CREATE SCHEMA verifier;

CREATE TABLE IF NOT EXISTS verifier.musig_identifier
(
    public_key VARCHAR(255) NOT NULL,
    rune_id VARCHAR(255) NOT NULL,
    is_issuer BOOLEAN NOT NULL,
    dkg_state JSON NOT NULL,
    PRIMARY KEY (public_key, rune_id)
);

CREATE TABLE IF NOT EXISTS verifier.sign_session
(
    public_key VARCHAR(255) NOT NULL,
    rune_id VARCHAR(255) NOT NULL,
    session_id VARCHAR(255) NOT NULL,
    tweak BYTEA NOT NULL,
    message_hash BYTEA NOT NULL,
    metadata JSON NOT NULL,
    sign_state JSON NOT NULL,
    PRIMARY KEY (session_id),
    FOREIGN KEY (public_key, rune_id) REFERENCES verifier.musig_identifier(public_key, rune_id)
);

COMMIT;
