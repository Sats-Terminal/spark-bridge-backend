BEGIN TRANSACTION;

CREATE SCHEMA IF NOT EXISTS verifier;

----------- USER IDENTIFIERS -----------

-- Dkg pregenerated shares
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


CREATE TABLE IF NOT EXISTS verifier.user_identifier
(
    user_uuid    UUID    NOT NULL,
    dkg_share_id UUID    NOT NULL,
--     todo: remove
    public_key   TEXT    NOT NULL,
    rune_id      TEXT    NOT NULL,
    is_issuer    BOOLEAN NOT NULL,
    PRIMARY KEY (user_uuid, rune_id),
    FOREIGN KEY (dkg_share_id) REFERENCES verifier.dkg_share (dkg_share_id)
);

------------ DEPOSIT_ADDRESS -----------

CREATE TABLE IF NOT EXISTS verifier.deposit_address
(
    nonce_tweak         BYTEA   NOT NULL,
    user_uuid           UUID    NOT NULL,
    rune_id             TEXT    NOT NULL,
    deposit_address     TEXT    NOT NULL,
    bridge_address      TEXT    NOT NULL,
    is_btc              BOOLEAN NOT NULL,
    deposit_amount      BIGINT  NOT NULL,
    confirmation_status JSON    NOT NULL,
    sats_fee_amount     BIGINT,
    out_point           TEXT,
    PRIMARY KEY (user_uuid, nonce_tweak),
    FOREIGN KEY (user_uuid, rune_id) REFERENCES verifier.user_identifier (user_uuid, rune_id)
);

CREATE INDEX IF NOT EXISTS user_identifier_index
    ON verifier.user_identifier (dkg_share_id)
    INCLUDE (user_uuid, public_key, rune_id, is_issuer);
CREATE INDEX IF NOT EXISTS user_identifier_2_index
    ON verifier.user_identifier (user_uuid, rune_id)
    INCLUDE (dkg_share_id, public_key, is_issuer);

CREATE INDEX IF NOT EXISTS sign_session_index
    ON verifier.sign_session (session_id)
    INCLUDE (dkg_share_id, tweak, message_hash, metadata, sign_state);

CREATE INDEX IF NOT EXISTS deposit_address_index
    ON verifier.deposit_address (user_uuid, rune_id, nonce_tweak)
    INCLUDE (deposit_address, bridge_address, is_btc, deposit_amount, confirmation_status, sats_fee_amount, out_point);
CREATE INDEX IF NOT EXISTS deposit_address_2_index
    ON verifier.deposit_address (deposit_address)
    INCLUDE (user_uuid, rune_id, nonce_tweak, bridge_address, is_btc, deposit_amount, confirmation_status, sats_fee_amount, out_point);

COMMIT;
