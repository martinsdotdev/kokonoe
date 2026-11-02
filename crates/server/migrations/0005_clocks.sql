-- A Room's time control and its clocks (KOK-57), spec §11. A timed game keeps, for each played
-- move, the time its mover had left after it, so a takeback restores both clocks; an untimed one
-- keeps none. turn_since is when the turn now running began (the second seat filling, the last
-- move, or a takeback), so a Room loaded again charges the player to move for the time between.
-- A timeout is a Termination, and an aborted game is deleted, so a rematch naming it lets go.
ALTER TABLE games
    ADD COLUMN clock text CHECK (clock IN ('3+2', '5+3', '10+5', '15+5')),
    ADD COLUMN clocks integer[] NOT NULL DEFAULT '{}',
    ADD COLUMN turn_since timestamptz,
    ADD CHECK (cardinality(clocks) = CASE WHEN clock IS NULL THEN 0 ELSE cardinality(moves) END),
    DROP CONSTRAINT games_termination_check,
    ADD CONSTRAINT games_termination_check CHECK (
        termination IN ('line', 'count', 'decided', 'resign', 'agreed draw', 'timeout')
    ),
    DROP CONSTRAINT games_rematch_fkey,
    ADD CONSTRAINT games_rematch_fkey
        FOREIGN KEY (rematch) REFERENCES games (room) ON DELETE SET NULL;
