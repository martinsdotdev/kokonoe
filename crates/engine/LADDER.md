# The Level ladder

Each of the computer's 8 Levels should beat the one below it 65–75% of the time, on Standard and on Deep (spec §6). This file is the record: the ladder as the code has it, and how its adjacent Levels fared against each other in self-play.

`cargo run --release --example ladder` measures it. Every Level plays as the browser plays it: below Level 5 one search of its whole budget, and from Level 5 four searches of a quarter each, as on a device with five or more cores, their visits summed for one choice. The clock never moves, so only the budgets bind. A pair plays batches of 50 games from the Preset's empty board, the higher Level taking X in half of them, a draw scoring half. It stops once the score's 95% Wilson interval lies wholly inside 65–75% (in band), wholly below it (too close) or wholly above it (too far apart), or at 400 games.

`--write` records a Preset's results and the ladder they were played on, and `tests/search.rs` holds the ladder below equal to `Level` in `src/search.rs`.

## The ladder

| Level | Iterations | Choice | Tactics | Workers |
| --: | --: | :-- | :-- | :-- |
| 1 | 30 | τ 2.0 | no | 1 |
| 2 | 100 | τ 2.0 | no | 1 |
| 3 | 300 | τ 1.0 | yes | 1 |
| 4 | 1,000 | τ 1.0 | yes | 1 |
| 5 | 3,000 | τ 0.5 | yes | up to 4 |
| 6 | 10,000 | τ 0.5 | yes | up to 4 |
| 7 | 30,000 | most visited | yes | up to 4 |
| 8 | 100,000 | most visited | yes | up to 4 |

## Standard

Not played yet.

## Deep

Not played yet.
