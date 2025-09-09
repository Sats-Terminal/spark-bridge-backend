CREATE TABLE IF NOT EXISTS gateway.utxo (
                                            id SERIAL PRIMARY KEY,
                                            txid TEXT NOT NULL,
                                            vout INT NOT NULL,
                                            amount BIGINT NOT NULL,
                                            rune_id TEXT NOT NULL,
                                            owner_address TEXT NOT NULL,
                                            nonce TEXT,
                                            status TEXT NOT NULL DEFAULT 'unspent',
                                            block_height BIGINT,
                                            lock_expires_at TIMESTAMP,
                                            created_at TIMESTAMP DEFAULT now(),
    updated_at TIMESTAMP DEFAULT now(),
    UNIQUE(txid, vout)
    );

CREATE INDEX IF NOT EXISTS idx_utxo_status ON gateway.utxo(status);
CREATE INDEX IF NOT EXISTS idx_utxo_txid_vout ON gateway.utxo(txid, vout);
