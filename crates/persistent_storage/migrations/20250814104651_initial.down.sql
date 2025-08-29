-- Add down migration script here
BEGIN TRANSACTION;

DROP TABLE IF EXISTS runes_spark.user_request_stats;
DROP TABLE IF EXISTS runes_spark.btc_indexer_work_checkpoint;

COMMIT;