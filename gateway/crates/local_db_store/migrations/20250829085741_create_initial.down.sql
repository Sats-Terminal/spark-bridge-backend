BEGIN TRANSACTION;

DROP TABLE IF EXISTS gateway.utxo;
DROP TABLE IF EXISTS gateway.session_requests;
DROP TABLE IF EXISTS gateway.deposit_address;
DROP TABLE IF EXISTS gateway.sign_session;
DROP TABLE IF EXISTS gateway.user_identifier;
DROP TABLE IF EXISTS gateway.dkg_share;

DROP SCHEMA IF EXISTS gateway CASCADE;

COMMIT;
