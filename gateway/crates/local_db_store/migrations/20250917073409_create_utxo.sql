CREATE TYPE UTXO_STATUS AS ENUM (
    'pending',
    'confirmed',
    'spent'
);

CREATE TABLE IF NOT EXISTS gateway.utxo
(
    out_point    TEXT        PRIMARY KEY,
    rune_amount       BIGINT      NOT NULL,
    rune_id      TEXT        NOT NULL,
    sats_fee_amount BIGINT   ,
    status       UTXO_STATUS NOT NULL DEFAULT 'pending',
    btc_address  TEXT        NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_utxo_status ON gateway.utxo (status);
