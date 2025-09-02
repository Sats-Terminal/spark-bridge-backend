BEGIN TRANSACTION;

CREATE SCHEMA gateway;

CREATE TABLE IF NOT EXISTS gateway.keys
(
    key_id UUID PRIMARY KEY
);

CREATE TABLE IF NOT EXISTS gatewayrequests
(
    request_id UUID PRIMARY KEY,
    key_id     UUID NOT NULL,
    FOREIGN KEY (key_id) REFERENCES gateway.keys (key_id)
);

COMMIT;
