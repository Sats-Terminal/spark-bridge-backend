BEGIN TRANSACTION;


CREATE SCHEMA runes_spark;

-- todo: remove
CREATE TYPE STATUS_TRANSFERRING AS ENUM ('created', 'processing', 'finished_success', 'finished_error');
CREATE TYPE STATUS_BTC_INDEXER AS ENUM ('created', 'processing', 'finished_success', 'finished_error');

-- todo: remove
CREATE TABLE IF NOT EXISTS runes_spark.user_request_stats
(
    uuid       UUID UNIQUE              NOT NULL,
    status     STATUS_TRANSFERRING      NOT NULL,
    error      TEXT,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL,
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL,
    PRIMARY KEY (uuid)
);

-- todo: remove
CREATE TABLE IF NOT EXISTS runes_spark.btc_indexer_work_checkpoint
(
    uuid         UUID UNIQUE              NOT NULL,
    status       STATUS_BTC_INDEXER       NOT NULL,
    task         JSONB                    NOT NULL,
    callback_url TEXT                     NOT NULL,
    error        TEXT,
    created_at   TIMESTAMP WITH TIME ZONE NOT NULL,
    updated_at   TIMESTAMP WITH TIME ZONE NOT NULL,
    PRIMARY KEY (uuid)
);

-- todo: remove
CREATE TABLE IF NOT EXISTS runes_spark.tx_ids_indexed
(
    tx_id         TEXT UNIQUE NOT NULL,
    block_height  INT,
    btc_tx_review JSONB,
    transaction   JSONB,
    PRIMARY KEY (tx_id)
);

CREATE TYPE BTC_TRACK_TX_REQUEST_STATUS AS ENUM ('pending', 'finished');
CREATE TYPE BTC_TRACKED_TX_STATUS AS ENUM ('pending', 'finalized');

CREATE TABLE btc_tracked_tx
(
    id            SERIAL PRIMARY KEY,
    tx_id         TEXT                  NOT NULL,
    v_out         INTEGER               NOT NULL,
    status        BTC_TRACKED_TX_STATUS NOT NULL,
    btc_tx_review JSONB,
    transaction   JSONB,
    created_at    TIMESTAMP             NOT NULL,
    UNIQUE (tx_id, v_out)
);

CREATE TABLE btc_track_tx_request
(
    uuid          UUID PRIMARY KEY,
    tracked_tx_id INTEGER                     NOT NULL REFERENCES btc_tracked_tx (id),
    callback_url  TEXT,
    created_at    TIMESTAMP                   NOT NULL,
    status        BTC_TRACK_TX_REQUEST_STATUS NOT NULL DEFAULT 'pending'
);



COMMIT;