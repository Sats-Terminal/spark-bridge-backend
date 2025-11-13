ALTER TABLE gateway.user_identifier
    ADD COLUMN IF NOT EXISTS external_user_id TEXT;

CREATE UNIQUE INDEX IF NOT EXISTS user_identifier_external_user_id_idx
    ON gateway.user_identifier (external_user_id)
    WHERE external_user_id IS NOT NULL;
