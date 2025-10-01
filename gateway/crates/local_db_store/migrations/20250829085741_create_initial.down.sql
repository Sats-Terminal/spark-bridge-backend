BEGIN TRANSACTION;

DROP INDEX IF EXISTS dkg_share_index;
DROP INDEX IF EXISTS dkg_share_2_index;
DROP INDEX IF EXISTS user_identifier_index;
DROP INDEX IF EXISTS user_identifier_2_index;
DROP INDEX IF EXISTS sign_session_index;
DROP INDEX IF EXISTS deposit_address_index;
DROP INDEX IF EXISTS deposit_address_2_index;

DROP TABLE IF EXISTS gateway.utxo;
DROP TABLE IF EXISTS gateway.session_requests;
DROP TABLE IF EXISTS gateway.deposit_address;
DROP TABLE IF EXISTS gateway.sign_session;
DROP TABLE IF EXISTS gateway.user_identifier;
DROP TABLE IF EXISTS gateway.dkg_share;

DROP SCHEMA IF EXISTS gateway CASCADE;

COMMIT;
