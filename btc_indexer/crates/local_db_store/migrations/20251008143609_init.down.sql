-- Add down migration script here

DROP TABLE IF EXISTS btc_indexer.watch_request;
DROP SCHEMA IF EXISTS btc_indexer;

DROP TYPE IF EXISTS WATCH_REQUEST_STATUS;
