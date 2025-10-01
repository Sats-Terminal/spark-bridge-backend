-- Add down migration script here
BEGIN TRANSACTION;

DROP INDEX IF EXISTS tx_tracking_indexed;
DROP INDEX IF EXISTS tx_tracking_requests_status_indexed;
DROP INDEX IF EXISTS tx_tracking_requests_uuid_indexed;

DROP TABLE IF EXISTS btc_indexer.tx_tracking_requests;
DROP TABLE IF EXISTS btc_indexer.tx_tracking;

DROP SCHEMA IF EXISTS btc_indexer;

COMMIT;