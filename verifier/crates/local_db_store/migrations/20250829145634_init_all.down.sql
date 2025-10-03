BEGIN TRANSACTION;

DROP TABLE IF EXISTS verifier.deposit_address;
DROP TABLE IF EXISTS verifier.sign_session;
DROP TABLE IF EXISTS verifier.musig_identifier;

COMMIT;