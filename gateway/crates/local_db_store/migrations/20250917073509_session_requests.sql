
CREATE TYPE REQ_TYPE AS ENUM (
    'get_runes_deposit_address',
    'get_spark_deposit_address',
    'bridge_runes',
    'exit_spark'
);

CREATE TYPE REQUEST_STATUS AS ENUM (
    'pending',
    'processing',
    'completed',
    'failed',
    'cancelled'
    );

CREATE TABLE IF NOT EXISTS gateway.session_requests
(
    session_id   UUID PRIMARY KEY,
    request_type REQ_TYPE       NOT NULL,
    request_status REQUEST_STATUS NOT NULL
);