-- Accounts and their sessions (spec §9, §17). Every Account is a Guest until sign-in arrives
-- (KOK-53). Guests' Handles needn't be unique; members' unique index comes with picking one.
CREATE TABLE accounts (
    id bigint GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    handle citext NOT NULL,
    created_at timestamptz NOT NULL DEFAULT now()
);

-- A session as Pilcrow's auth book has it: the cookie holds the id and a secret, and only the
-- secret's SHA-256 is kept here.
CREATE TABLE sessions (
    id text PRIMARY KEY,
    secret_hash bytea NOT NULL,
    account_id bigint NOT NULL REFERENCES accounts ON DELETE CASCADE,
    expires_at timestamptz NOT NULL
);

CREATE INDEX sessions_account_id ON sessions (account_id);
