DROP INDEX IF EXISTS user_identifier_external_user_id_idx;

ALTER TABLE gateway.user_identifier
    DROP COLUMN IF EXISTS external_user_id;
