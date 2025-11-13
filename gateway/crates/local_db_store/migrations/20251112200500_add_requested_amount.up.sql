ALTER TABLE gateway.deposit_address
    ADD COLUMN IF NOT EXISTS requested_amount BIGINT;

UPDATE gateway.deposit_address
SET requested_amount = amount
WHERE requested_amount IS NULL;

ALTER TABLE gateway.deposit_address
    ALTER COLUMN requested_amount SET NOT NULL;
