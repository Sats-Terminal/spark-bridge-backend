BEGIN TRANSACTION;


CREATE SCHEMA runes_spark;
CREATE TYPE STATUS_TRANSFERRING AS ENUM ('created', 'processing', 'finished_success', 'finished_error');
CREATE TYPE STATUS_BTC_INDEXER AS ENUM ('created', 'processing', 'finished_success', 'finished_error');

CREATE TABLE IF NOT EXISTS runes_spark.user_request_stats
(
    uuid       UUID UNIQUE              NOT NULL,
    status     STATUS_TRANSFERRING      NOT NULL,
    error      TEXT,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL,
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL,
    PRIMARY KEY (uuid)
);

CREATE TABLE IF NOT EXISTS runes_spark.btc_indexer_work_checkpoint
(
    uuid         UUID UNIQUE              NOT NULL,
    status       STATUS_BTC_INDEXER       NOT NULL,
    task         JSONB                    NOT NULL,
    created_at   TIMESTAMP WITH TIME ZONE NOT NULL,
    callback_url TEXT NOT NULL,
    error        TEXT,
    updated_at   TIMESTAMP WITH TIME ZONE NOT NULL,
    PRIMARY KEY (uuid)
);

COMMIT;