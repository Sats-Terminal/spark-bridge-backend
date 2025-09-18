CREATE TYPE UTXO_STATUS AS ENUM (
    'pending',
    'confirmed',
    'spent'
);

CREATE TABLE IF NOT EXISTS gateway.utxo
(
    txid         TEXT        NOT NULL,
    vout         INT         NOT NULL,
    amount       BIGINT      NOT NULL,
    rune_id      TEXT        NOT NULL,
    sats_fee_amount BIGINT   ,
    status       UTXO_STATUS NOT NULL DEFAULT 'pending',
    btc_address  TEXT        NOT NULL,
    transaction  JSONB,
    PRIMARY KEY (txid, vout)
);

CREATE INDEX IF NOT EXISTS idx_utxo_status ON gateway.utxo (status);
