# Bumping RULES_VERSION

A Record is only a Ruleset, a Seed and the moves. The position, the dealt opening and the result all come from replaying it, so every stored game, share link and imported file depends on the engine replaying it the way it was played (ADR-0004, spec §3.4). `RULES_VERSION` in `src/lib.rs` names the rules a Record was played under, and `Game::open` replays only its own version.

## The golden corpus

`tests/corpus/v<N>/` holds games played under version N, one JSON file each. A file has four keys:

- `record`: the Record, as spec §4.3 writes it.
- `opening`: the Cells its Seed dealt.
- `outcome`: how the game ended, or `null` while it goes on.
- `turns`: one line for each ply that has a side to move, opening plies included. Each line holds where the mover was Sent (`to 4`, `free: 47 within 4`, a Board written as its Space digits) and the legal Cells there, as indices, in the engine's order.

`tests/corpus.rs` checks the corpus on every build, with three tests:

- `every_record_replays_as_it_was_recorded` replays every file. Any change to the deal, a Sent target, a legal Cell, their order or the outcome fails it, naming the file and the ply.
- `every_game_the_writer_knows_has_its_file` fails if a game in the writer's lists has no file in the current slice.
- `the_corpus_covers_the_rules` fails if no game is left for a case. The cases are every Preset with each Outcome it can reach, both depths, a Random opening at each depth, every Termination the engine decides, a game over inside its opening, and an unfinished game.

## When the corpus fails

First decide whether the change was meant to change the rules.

- **It wasn't.** For example, a refactor reordered the legal Cells or touched the deal. Fix the code, not the corpus.
- **It was.** For example, a rules fix, a new opening generator, or a new legal-move order. Bump the version as below. Never edit or delete a corpus file to make the test pass: those files are what old games need to keep replaying.

## Bumping from N to N + 1

1. Copy only the pieces that change into a `frozen::vN` module, unchanged: a function, a table or a generator. Everything else stays shared.
2. Make `Game::open` accept every version from 1 to N + 1, and use the frozen pieces when `record.rules_version` is N. A game played under N then replays under N's rules forever.
3. Make the change in the current code, and set `RULES_VERSION` to N + 1. The corpus tests now fail, because `vN+1/` doesn't exist yet. That's expected until step 4.
4. Run `cargo test --test corpus -- --ignored` to write `tests/corpus/vN+1/`.
   - **Named games** replay their moves under the new rules. If one's moves are no longer legal, the writer stops at it. Replace its moves with a game that shows the same thing under the new rules.
   - **Searched games** are found again, so they can be different games from `vN/`'s. If a search finds nothing, change what it looks for.
   - Check that the diffs between the new files and `vN/` show only what the change meant to change.
5. Leave `tests/corpus/vN/` as it is. It now replays through the frozen pieces, which keeps them tested.
6. Check that `cargo test --workspace` passes.

The browser runs the same engine, compiled by `node scripts/bindings.mjs`. Its saved games, imported files and share links each carry their `rules_version`, so they replay through step 2's frozen pieces as well. Its tests mirror a few engine games in `web/src/lib/game/games.fixtures.ts`, so update any that changed. A client on version N can't open a game played under N + 1, so ship the bump to every client before the server starts new games under it.

## Adding a game without a bump

To cover something new, add the game to the writer's lists in `tests/corpus.rs`. Put moves found elsewhere in `NAMED`. Put a final position that random play should find in `SEARCHED`. Then run `cargo test --test corpus -- --ignored`. The writer only adds missing files and never rewrites one, so it can't hide a rules change.
