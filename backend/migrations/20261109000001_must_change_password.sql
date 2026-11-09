-- Set for an account whose password someone else knows: the owner account
-- generated on first start (its password is printed to the log). Such a user
-- may only change the password until it is cleared.
ALTER TABLE users ADD COLUMN must_change_password BOOLEAN NOT NULL DEFAULT false;
