BEGIN TRANSACTION;

CREATE SCHEMA verifier;

CREATE TABLE IF NOT EXISTS user_state
(
    user_public_key VARCHAR(255) NOT NULL,
    state_data VARCHAR(255) NOT NULL,
    PRIMARY KEY (user_public_key)
);

COMMIT;