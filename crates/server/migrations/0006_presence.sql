-- Presence and the endings it brings (KOK-58), spec §10.3 and §11. A seated player's away is when
-- their last socket in the Room went, null while they have one and for an empty seat, so a Room
-- loaded again knows how long each has been gone: a Claim waits on the other player's, and a game
-- both players left a day ago ends Abandoned, with no Result and its moves kept. A Claim is a
-- Termination like any other.
ALTER TABLE games
    ADD COLUMN x_away timestamptz,
    ADD COLUMN o_away timestamptz,
    ADD CONSTRAINT games_x_away_check CHECK (x_away IS NULL OR x_id IS NOT NULL),
    ADD CONSTRAINT games_o_away_check CHECK (o_away IS NULL OR o_id IS NOT NULL),
    DROP CONSTRAINT games_termination_check,
    ADD CONSTRAINT games_termination_check CHECK (
        termination IN ('line', 'count', 'decided', 'resign', 'agreed draw', 'timeout', 'claim',
                        'abandoned')
    ),
    -- 0004's (termination IS NULL OR result IS NOT NULL), which an abandonment breaks.
    DROP CONSTRAINT games_check2,
    -- Every ending has a Result but an abandonment, which has none. A game that ended before
    -- Terminations were kept has a Result and no Termination.
    ADD CONSTRAINT games_ending_check CHECK (
        termination IS NULL OR (result IS NULL) = (termination = 'abandoned')
    );

-- The games the abandonment sweep reads, which the finished ones outnumber.
CREATE INDEX games_unfinished ON games (room) WHERE result IS NULL AND termination IS NULL;
