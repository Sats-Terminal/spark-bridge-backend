BEGIN TRANSACTION;

CREATE TABLE IF NOT EXISTS gateway.rune_metadata_map
(
    rune_id TEXT PRIMARY KEY,
    rune_metadata JSONB,
    wrune_metadata JSONB NOT NULL,
    issuer_public_key TEXT NOT NULL,
    bitcoin_network TEXT NOT NULL,
    spark_network TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

COMMIT;
