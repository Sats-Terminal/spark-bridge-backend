ALTER TABLE verifier.deposit_address
    DROP CONSTRAINT IF EXISTS deposit_address_pkey;

ALTER TABLE verifier.deposit_address
    ADD CONSTRAINT deposit_address_pkey PRIMARY KEY (dkg_share_id);
