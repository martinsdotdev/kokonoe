-- Games played in invite Rooms (spec §10, §17), one row a Room. Each move is appended before it's
-- acknowledged, so a restart rebuilds every Room from here. A seat is null until someone takes
-- it, and the Result is null while the game goes on. Clocks, Rated play, Room options, the
-- Termination and visibility come with the tickets that use them.
CREATE TABLE games (
    id bigint GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    room text NOT NULL UNIQUE CHECK (room ~ '^[0-9a-f]{20}$'),
    rules_version integer NOT NULL,
    ruleset jsonb NOT NULL,
    ruleset_code text NOT NULL,
    -- Spec §4.3's 16 hex digits: a u64 doesn't fit a bigint.
    seed text NOT NULL CHECK (seed ~ '^[0-9a-f]{16}$'),
    -- Cell indices, after the dealt opening.
    moves integer[] NOT NULL DEFAULT '{}',
    host_id bigint NOT NULL REFERENCES accounts,
    x_id bigint REFERENCES accounts,
    o_id bigint REFERENCES accounts,
    result text CHECK (result IN ('1-0', '0-1', '1/2-1/2')),
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now(),
    CHECK (host_id IN (x_id, o_id)),
    CHECK (x_id <> o_id)
);

CREATE INDEX games_x_id ON games (x_id);
CREATE INDEX games_o_id ON games (o_id);
