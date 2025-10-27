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
    dkg_share_id UUID    NOT NULL,
    user_id    UUID    NOT NULL,
    rune_id      TEXT    NOT NULL,
    is_issuer    BOOLEAN NOT NULL,
    PRIMARY KEY (dkg_share_id),
    FOREIGN KEY (dkg_share_id) REFERENCES verifier.dkg_share (dkg_share_id)
);

------------ DEPOSIT_ADDRESS -----------

CREATE TABLE IF NOT EXISTS verifier.deposit_address
(
    nonce_tweak         BYTEA   NOT NULL,
    dkg_share_id        UUID    NOT NULL,
    deposit_address     TEXT    NOT NULL,
    bridge_address      TEXT    NOT NULL,
    is_btc              BOOLEAN NOT NULL,
    deposit_amount      BIGINT  NOT NULL,
    confirmation_status JSON    NOT NULL,
    sats_fee_amount     BIGINT,
    outpoint           TEXT,
    PRIMARY KEY (dkg_share_id),
    FOREIGN KEY (dkg_share_id) REFERENCES verifier.user_identifier (dkg_share_id)
);

COMMIT;
