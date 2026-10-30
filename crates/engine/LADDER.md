# The Level ladder

Each of the computer's 8 Levels should beat the one below it 65–75% of the time, on Standard and on Deep (spec §6). This file is the record: the ladder as the code has it, and how its adjacent Levels fared against each other in self-play.

`cargo run --release --example ladder` measures it. Every Level plays as the browser plays it: below Level 5 one search of its whole budget, and from Level 5 four searches of a quarter each, as on a device with five or more cores, their visits summed for one choice. The clock never moves, so only the budgets bind. A pair plays batches of 50 games from the Preset's empty board, the higher Level taking X in half of them, a draw scoring half. It stops once the score's 95% Wilson interval lies wholly inside 65–75% (in band), wholly below it (too close) or wholly above it (too far apart), or at 400 games. Checking after every batch makes the 95% approximate. At 400 games the interval still spans about 9 points, so only a score close to 70% settles inside a 10-point band; a pair still undecided at 400 is named by where its score sits ("in band, unsettled").

`--write` records a Preset's results and the ladder they were played on, and `tests/search.rs` holds the ladder below equal to `Level` in `src/search.rs`.

## The ladder

| Level | Iterations | Choice | Tactics | Workers |
| --: | --: | :-- | :-- | :-- |
| 1 | 30 | most visited | no | 1 |
| 2 | 100 | τ 0.15 | no | 1 |
| 3 | 300 | τ 0.25 | yes | 1 |
| 4 | 1,000 | τ 0.25 | yes | 1 |
| 5 | 3,000 | τ 0.1 | yes | up to 4 |
| 6 | 10,000 | τ 0.1 | yes | up to 4 |
| 7 | 30,000 | most visited | yes | up to 4 |
| 8 | 100,000 | most visited | yes | up to 4 |

## Standard

Played by `cargo run --release --example ladder -- --preset std --pairs 1-8 --write`.

| Pair | Games | Won | Drawn | Lost | Score | 95% interval | Verdict | Plies a game |
| :-- | --: | --: | --: | --: | --: | :-- | :-- | --: |
| 2 over 1 | 400 | 264 | 45 | 91 | 71.6% | 67.0%–75.8% | in band, unsettled | 52 |
| 3 over 2 | 400 | 249 | 79 | 72 | 72.1% | 67.5%–76.3% | in band, unsettled | 52 |
| 4 over 3 | 400 | 253 | 94 | 53 | 75.0% | 70.5%–79.0% | in band, unsettled | 53 |
| 5 over 4 | 400 | 245 | 95 | 60 | 73.1% | 68.6%–77.2% | in band, unsettled | 51 |
| 6 over 5 | 400 | 244 | 94 | 62 | 72.8% | 68.2%–76.9% | in band, unsettled | 50 |
| 7 over 6 | 400 | 253 | 85 | 62 | 73.9% | 69.4%–77.9% | in band, unsettled | 49 |
| 8 over 7 | 400 | 225 | 104 | 71 | 69.2% | 64.6%–73.6% | in band, unsettled | 49 |

## Deep

Played on the ladder above by `cargo run --release --example ladder -- --preset d3 --pairs 1-8 --write`, which stopped during 8 over 7 when the machine ran short of memory; the six pairs it finished are copied from its output.

| Pair | Games | Won | Drawn | Lost | Score | 95% interval | Verdict | Plies a game |
| :-- | --: | --: | --: | --: | --: | :-- | :-- | --: |
| 2 over 1 | 50 | 18 | 10 | 22 | 46.0% | 33.0%–59.6% | too close | 449 |
| 3 over 2 | 50 | 39 | 11 | 0 | 89.0% | 77.4%–95.0% | too far apart | 469 |
| 4 over 3 | 400 | 285 | 63 | 52 | 79.1% | 74.9%–82.8% | too far apart, unsettled | 453 |
| 5 over 4 | 50 | 43 | 6 | 1 | 92.0% | 81.2%–96.8% | too far apart | 438 |
| 6 over 5 | 50 | 48 | 2 | 0 | 98.0% | 89.5%–99.6% | too far apart | 399 |
| 7 over 6 | 50 | 50 | 0 | 0 | 100.0% | 92.9%–100.0% | too far apart | 246 |
| 8 over 7 | | | | | | | not played | |

The ladder tuned on Standard doesn't fit Deep. A Deep game runs 250–470 plies against Standard's 50, so a small edge in each move compounds, and a free move has up to 81 Cells, so 30 and 100 iterations both play close to random. Levels 3 to 7 each crush the one below, and Level 2 doesn't beat Level 1. Fitting Deep needs its own iterations and τ for each Level, as its caps already differ; KOK-43 stays open for that.
