-- Add up migration script here

CREATE TABLE IF NOT EXISTS keys (
    key_id UUID NOT NULL PRIMARY KEY,
    metadata VARCHAR(255) NOT NULL
);
