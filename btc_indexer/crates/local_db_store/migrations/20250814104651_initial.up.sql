BEGIN TRANSACTION;


CREATE SCHEMA IF NOT EXISTS btc_indexer;

CREATE TYPE BTC_TRACK_TX_REQUEST_STATUS AS ENUM ('pending', 'finished', 'failed_to_send');
CREATE TYPE BTC_TRACKED_TX_STATUS AS ENUM ('pending', 'finalized');

CREATE TABLE btc_indexer.tx_tracking
(
    id            SERIAL PRIMARY KEY,
    tx_id         TEXT                  NOT NULL,
    v_out         INTEGER               NOT NULL,
    amount        BIGINT                NOT NULL,
    rune_id       TEXT                  NOT NULL,
    status        BTC_TRACKED_TX_STATUS NOT NULL,
    btc_tx_review JSONB,
    transaction   JSONB,
    created_at    TIMESTAMP             NOT NULL DEFAULT NOW(),
    UNIQUE (tx_id, v_out)
);

CREATE TABLE btc_indexer.tx_tracking_requests
(
    uuid          UUID PRIMARY KEY,
    tracked_tx_id INTEGER                     NOT NULL REFERENCES btc_indexer.tx_tracking (id),
    callback_url  TEXT,
    created_at    TIMESTAMP                   NOT NULL DEFAULT NOW(),
    status        BTC_TRACK_TX_REQUEST_STATUS NOT NULL DEFAULT 'pending'
);

CREATE INDEX IF NOT EXISTS tx_tracking_indexed
    ON btc_indexer.tx_tracking (status);
CREATE INDEX IF NOT EXISTS tx_tracking_requests_status_indexed
    ON btc_indexer.tx_tracking_requests (status)
    INCLUDE (uuid, tracked_tx_id, callback_url, created_at);


CREATE INDEX IF NOT EXISTS tx_tracking_requests_uuid_indexed
    ON btc_indexer.tx_tracking_requests (uuid);

COMMIT;