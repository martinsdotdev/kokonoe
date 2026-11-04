-- Spectators (KOK-61), spec §10.1 and §10.2. A host chooses whether their invite Room can be
-- watched, off unless they ask: an invite Room's game is its two players' (§14). A Room made
-- before this was made without being asked, so it stays closed to Spectators. A rematch's Room
-- copies the choice with the other options.
ALTER TABLE games ADD COLUMN spectators boolean NOT NULL DEFAULT false;
