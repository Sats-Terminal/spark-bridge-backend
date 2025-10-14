BEGIN TRANSACTION;

DROP INDEX IF EXISTS user_identifier_index;
DROP INDEX IF EXISTS user_identifier_2_index;
DROP INDEX IF EXISTS sign_session_index;
DROP INDEX IF EXISTS deposit_address_index;
DROP INDEX IF EXISTS deposit_address_2_index;

DROP TABLE IF EXISTS verifier.deposit_address;
DROP TABLE IF EXISTS verifier.user_identifier;
DROP TABLE IF EXISTS verifier.sign_session;
DROP TABLE IF EXISTS verifier.dkg_share;
DROP TABLE IF EXISTS verifier.sessions;

DROP TYPE IF EXISTS DEPOSIT_STATUS;
DROP TYPE IF EXISTS REQUEST_TYPE;
DROP TYPE IF EXISTS REQUEST_STATUS;

DROP SCHEMA IF EXISTS verifier;

COMMIT;