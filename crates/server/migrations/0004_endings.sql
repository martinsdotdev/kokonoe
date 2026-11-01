-- How a Room's game ends besides by the rules, and the option a Room was made with (KOK-60).
-- Termination is spec §4.4's, null while the game goes on (or for a game that ended before this
-- column, whose rules ending the engine can tell again). Each player may offer 3 draws a game,
-- counted here so a reconnect can't reset them.
ALTER TABLE games
    ADD COLUMN takebacks boolean NOT NULL DEFAULT false,
    ADD COLUMN termination text
        CHECK (termination IN ('line', 'count', 'decided', 'resign', 'agreed draw')),
    ADD COLUMN x_draw_offers smallint NOT NULL DEFAULT 0 CHECK (x_draw_offers BETWEEN 0 AND 3),
    ADD COLUMN o_draw_offers smallint NOT NULL DEFAULT 0 CHECK (o_draw_offers BETWEEN 0 AND 3),
    ADD CHECK (termination IS NULL OR result IS NOT NULL);
