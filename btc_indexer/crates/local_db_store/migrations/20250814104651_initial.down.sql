-- Add down migration script here
BEGIN TRANSACTION;

DROP TABLE IF EXISTS btc_indexer.tx_tracking_requests;
DROP TABLE IF EXISTS btc_indexer.tx_tracking;

DROP SCHEMA IF EXISTS btc_indexer;

COMMIT;