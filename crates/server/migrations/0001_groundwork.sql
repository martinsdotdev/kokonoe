-- What later tables need: citext, so an email or a Handle compares without regard to case
-- (spec §17). Each table arrives with the feature that uses it.
CREATE EXTENSION IF NOT EXISTS citext;
