-- Add up migration script here
CREATE TABLE IF NOT EXISTS keys (
    key_id UUID PRIMARY KEY
);

CREATE TABLE IF NOT EXISTS requests (
    request_id UUID PRIMARY KEY,
    key_id UUID NOT NULL,
    FOREIGN KEY (key_id) REFERENCES keys(key_id)
);
