BEGIN TRANSACTION;

DROP TABLE IF EXISTS verifier.deposit_address;
DROP TABLE IF EXISTS verifier.user_identifier;
DROP TABLE IF EXISTS verifier.sign_session;
DROP TABLE IF EXISTS verifier.dkg_share;

DROP SCHEMA IF EXISTS verifier;

COMMIT;