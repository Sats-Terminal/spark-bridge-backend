DROP INDEX IF EXISTS user_identifier_external_user_id_idx;

CREATE UNIQUE INDEX IF NOT EXISTS user_identifier_external_user_id_rune_idx
    ON gateway.user_identifier (external_user_id, rune_id)
    WHERE external_user_id IS NOT NULL;
