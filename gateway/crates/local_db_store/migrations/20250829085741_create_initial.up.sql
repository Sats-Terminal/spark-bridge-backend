BEGIN TRANSACTION;

CREATE SCHEMA gateway;

CREATE TABLE IF NOT EXISTS gateway.musig_identifier
(
    public_key VARCHAR(255) NOT NULL,
    rune_id    VARCHAR(255) NOT NULL,
    is_issuer  BOOLEAN      NOT NULL,
    dkg_state  JSON         NOT NULL,
    PRIMARY KEY (public_key, rune_id)
);

CREATE TABLE IF NOT EXISTS gateway.sign_session
(
    session_id   VARCHAR(255) NOT NULL,
    public_key   VARCHAR(255) NOT NULL,
    rune_id      VARCHAR(255) NOT NULL,
    tweak        BYTEA        NOT NULL,
    message_hash BYTEA        NOT NULL,
    metadata     JSON         NOT NULL,
    sign_state   JSON         NOT NULL,
    PRIMARY KEY (session_id),
    FOREIGN KEY (public_key, rune_id) REFERENCES gateway.musig_identifier (public_key, rune_id)
);

COMMIT;

