BEGIN TRANSACTION;

CREATE TABLE IF NOT EXISTS user_key_info
(
    user_public_key VARCHAR(255) PRIMARY KEY,
    state_data JSON NOT NULL
);

CREATE TABLE IF NOT EXISTS user_session_info
(
    user_public_key VARCHAR(255) NOT NULL,
    session_id VARCHAR(255) NOT NULL,
    tweak BYTEA NOT NULL,
    message_hash BYTEA NOT NULL,
    metadata JSON NOT NULL,
    state_data JSON NOT NULL,
    PRIMARY KEY (user_public_key, session_id)
);

COMMIT;
