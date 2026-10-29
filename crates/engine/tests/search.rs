//! The computer opponent (spec §6): its ladder, games at every Level finishing, the time cap,
//! the root-parallel split from Level 5, and, from Level 3, never missing an immediate win or
//! handing the opponent one.

mod common;
mod model;

use kokonoe_engine::{
    Cell, Choice, Depth, Game, Level, ParseError, Player, Preset, Ruleset, Searched, Seed, State,
    Status, search, shares, think,
};

use common::{Xorshift, advance, cells, game};

const DEPTH_TWO: [Preset; 4] = [
    Preset::Standard,
    Preset::ForcedContinuation,
    Preset::TicTacKu,
    Preset::CountTiebreak,
];

fn level(n: u8) -> Level {
    Level::try_from(n).unwrap()
}

/// A clock that never moves, so only the iteration budget binds.
fn frozen() -> u32 {
    0
}

fn legal(state: &State) -> Vec<Cell> {
    match state.status() {
        Status::ToMove { moves, .. } => moves.iter().collect(),
        Status::Over(_) => Vec::new(),
    }
}

/// The moves that end the game at once with `winner` winning.
fn ending_with(state: &State, winner: Player) -> Vec<Cell> {
    legal(state)
        .into_iter()
        .filter(|&cell| {
            let mut next = *state;
            matches!(next.play(cell), Ok(Status::Over(outcome)) if outcome.winner() == Some(winner))
        })
        .collect()
}

/// The moves that win the game at once for the side to move.
fn wins(state: &State) -> Vec<Cell> {
    ending_with(state, state.ply().player())
}

/// Whether `cell` hands the opponent the game: it wins on this move, or has a winning reply.
fn hands_over(state: &State, cell: Cell) -> bool {
    let opponent = state.ply().player().opponent();
    let mut next = *state;
    match next.play(cell).unwrap() {
        Status::Over(outcome) => outcome.winner() == Some(opponent),
        Status::ToMove { .. } => !wins(&next).is_empty(),
    }
}

/// Every position of 300 seeded random games under `rules`, in a fixed order.
fn positions(rules: Ruleset) -> impl Iterator<Item = State> {
    (1..=300).flat_map(move |seed| {
        let mut game = Game::new(rules, Seed(seed));
        let mut rng = Xorshift(seed);
        let mut states = Vec::new();
        while let Status::ToMove { moves, .. } = game.state().status() {
            states.push(*game.state());
            advance(&mut game, moves.nth(rng.below(moves.len())));
        }
        states
    })
}

/// Up to `per` positions that `keep` picks, under each of the 80 depth-2 Rulesets, so the tactics
/// meet every objective, fallback and drawn-board owner.
fn sample<T>(per: usize, keep: impl Fn(State) -> Option<T>) -> Vec<T> {
    model::depth_two_rulesets()
        .into_iter()
        .flat_map(|rules| positions(rules).filter_map(&keep).take(per))
        .collect()
}

/// The moves that hand the opponent the game, when some move doesn't and nothing wins at once.
fn avoidable_losses(state: State) -> Option<(State, Vec<Cell>)> {
    if !wins(&state).is_empty() {
        return None;
    }
    let moves = legal(&state);
    let losing: Vec<Cell> = moves
        .iter()
        .copied()
        .filter(|&cell| hands_over(&state, cell))
        .collect();
    (!losing.is_empty() && losing.len() < moves.len()).then_some((state, losing))
}

/// Level 3 and 4 never play a move of `losing`, over a few seeds.
fn avoids(state: &State, losing: &[Cell]) {
    for n in [3, 4] {
        for seed in 0..2 {
            let cell = think(state, level(n), seed, frozen).unwrap();
            assert!(!losing.contains(&cell), "Level {n} played {cell:?}");
        }
    }
}

/// Spec §6's caps and Workers. The rest of each rung, which self-play tunes, is held to
/// `LADDER.md` by `the_calibrated_ladder_is_the_one_in_code`.
#[test]
fn the_caps_and_workers_are_spec_section_6() {
    let ladder: Vec<_> = (Level::MIN..=Level::MAX)
        .map(|n| {
            let level = level(n);
            (
                level.cap_ms(Depth::Two),
                level.cap_ms(Depth::Three),
                // On an eight-core device.
                level.workers(8),
            )
        })
        .collect();
    assert_eq!(
        ladder,
        [
            (1_000, 3_000, 1),
            (1_000, 3_000, 1),
            (1_000, 3_000, 1),
            (1_000, 3_000, 1),
            (2_000, 6_000, 4),
            (2_000, 6_000, 4),
            (3_000, 9_000, 4),
            (3_000, 9_000, 4),
        ]
    );
    assert_eq!(Level::try_from(0), Err(ParseError::Level(0)));
    assert_eq!(Level::try_from(9), Err(ParseError::Level(9)));
}

#[test]
fn games_at_levels_one_to_four_finish_under_every_depth_two_preset() {
    for preset in DEPTH_TWO {
        for n in 1..=4 {
            let mut game = Game::new(preset.ruleset(), Seed(n.into()));
            let mut seed = u64::from(n);
            while let Some(cell) = think(game.state(), level(n), seed, frozen) {
                advance(&mut game, cell);
                seed += 1;
            }
            assert!(
                matches!(game.state().status(), Status::Over(_)),
                "{preset:?} at Level {n}"
            );
        }
    }
}

#[test]
fn a_finished_game_has_no_move() {
    let game = game(
        Preset::Standard.ruleset(),
        "03 30 04 40 05 51 13 31 14 41 15 52 26 62 27 72 28",
    );
    assert!(think(game.state(), level(4), 0, frozen).is_none());
}

#[test]
fn the_time_cap_stops_the_search() {
    let game = Game::new(Preset::Standard.ruleset(), Seed(0));
    let mut reads = 0;
    let cell = think(game.state(), level(8), 1, || {
        reads += 1;
        reads
    });
    assert!(cell.is_some());
    // A millisecond per read: the 3,000th reaches Level 8's 3 s cap at depth 2, long before
    // its 100,000 iterations would take 6,250 reads.
    assert_eq!(reads, 3_000);
}

/// The ladder as `LADDER.md` records it: each row's Level, iterations, choice and tactics.
fn recorded_ladder() -> Vec<(u8, u32, Choice, bool)> {
    let file = include_str!("../LADDER.md");
    let table = file
        .split("## The ladder")
        .nth(1)
        .and_then(|rest| rest.split("\n## ").next())
        .expect("LADDER.md has the ladder");
    table
        .lines()
        .map(|line| line.split('|').map(str::trim).collect::<Vec<_>>())
        // The rows that start with a Level, past the header and its rule.
        .filter(|cells| cells.get(1).is_some_and(|cell| cell.parse::<u8>().is_ok()))
        .map(|cells| {
            let row = cells.join(" | ");
            let choice = match cells[3].strip_prefix("τ ") {
                Some(tau) => Choice::Sample {
                    tau: tau.parse().unwrap_or_else(|_| panic!("a τ in {row}")),
                },
                None => {
                    assert_eq!(cells[3], "most visited", "a choice in {row}");
                    Choice::MostVisited
                }
            };
            (
                cells[1].parse().unwrap(),
                cells[2]
                    .replace(',', "")
                    .parse()
                    .unwrap_or_else(|_| panic!("iterations in {row}")),
                choice,
                cells[4] == "yes",
            )
        })
        .collect()
}

#[test]
fn the_calibrated_ladder_is_the_one_in_code() {
    let code: Vec<_> = (Level::MIN..=Level::MAX)
        .map(|n| {
            let level = level(n);
            (n, level.iterations(), level.choice(), level.tactics())
        })
        .collect();
    assert_eq!(recorded_ladder(), code, "LADDER.md and Level disagree");
}

#[test]
fn from_level_five_the_budget_splits_into_even_shares_that_sum_to_it() {
    for n in Level::MIN..=Level::MAX {
        let level = level(n);
        for cores in 0..=64 {
            let shares = shares(level, cores);
            let workers = if n < 5 {
                1
            } else {
                cores.saturating_sub(1).clamp(1, 4)
            };
            assert_eq!(shares.len(), workers as usize, "Level {n}, {cores} cores");
            assert_eq!(shares.iter().sum::<u32>(), level.iterations());
            let (most, least) = (shares.iter().max(), shares.iter().min());
            assert!(most.unwrap() - least.unwrap() <= 1, "{shares:?}");
        }
    }
}

/// The visits one search found, summed.
fn visited(searched: Option<Searched>) -> u32 {
    match searched {
        Some(Searched::Visits(visits)) => visits.iter().map(|&(_, n)| n).sum(),
        other => panic!("a search, not {other:?}"),
    }
}

#[test]
fn one_to_four_workers_search_the_same_total() {
    let level = level(5);
    for preset in [Preset::Standard, Preset::Deep] {
        let game = Game::new(preset.ruleset(), Seed(0));
        // Two to five cores give one to four Workers.
        for cores in 2..=5 {
            let total: u32 = shares(level, cores)
                .into_iter()
                .zip(0..)
                .map(|(share, seed)| visited(search(game.state(), level, share, seed, frozen)))
                .sum();
            assert_eq!(total, level.iterations(), "{preset:?}, {cores} cores");
        }
    }
}

#[test]
fn a_search_lists_every_root_move_in_cell_order() {
    let game = Game::new(Preset::Standard.ruleset(), Seed(0));
    let Some(Searched::Visits(visits)) = search(game.state(), level(1), 30, 0, frozen) else {
        panic!("a search");
    };
    let listed: Vec<Cell> = visits.iter().map(|&(cell, _)| cell).collect();
    assert_eq!(listed, legal(game.state()), "all 81, unvisited ones too");
    assert_eq!(visits.iter().map(|&(_, n)| n).sum::<u32>(), 30);
}

#[test]
fn a_search_that_has_one_move_to_play_says_so_at_once() {
    // O's 8·3 Sends X to Board 3, where 3·9 wins it and the top row.
    let game = game(
        Preset::Standard.ruleset(),
        &format!("{ONE_FROM_THE_TOP_ROW} 72"),
    );
    let mut reads = 0;
    let searched = search(game.state(), level(8), 25_000, 0, || {
        reads += 1;
        0
    });
    assert_eq!(searched, Some(Searched::Forced(cells("28")[0])));
    assert_eq!(reads, 0, "no search ran");
    let over = game_over();
    assert_eq!(search(over.state(), level(8), 25_000, 0, frozen), None);
}

fn game_over() -> Game {
    game(
        Preset::Standard.ruleset(),
        "03 30 04 40 05 51 13 31 14 41 15 52 26 62 27 72 28",
    )
}

/// X has won Boards 1 and 2 and holds 3·7 and 3·8, and O is Sent to Board 8.
const ONE_FROM_THE_TOP_ROW: &str = "03 30 04 40 05 51 13 31 14 41 15 52 26 62 27";

#[test]
fn level_three_completes_the_line() {
    // O's 8·3 Sends X to Board 3.
    let game = game(
        Preset::Standard.ruleset(),
        &format!("{ONE_FROM_THE_TOP_ROW} 72"),
    );
    for seed in 0..10 {
        assert_eq!(
            think(game.state(), level(3), seed, frozen),
            Some(cells("28")[0]),
            "3·9 wins Board 3, and the top row"
        );
    }
}

#[test]
fn level_three_never_sends_into_a_lost_board_and_level_one_can() {
    // O must move in Board 8. Its first three Cells Send X to Board 3, where X wins, or to
    // Boards 1 and 2, which X has won, for a Free move that reaches Board 3 too.
    let game = game(Preset::Standard.ruleset(), ONE_FROM_THE_TOP_ROW);
    let losing = cells("70 71 72");
    for seed in 0..10 {
        let cell = think(game.state(), level(3), seed, frozen).unwrap();
        assert!(!losing.contains(&cell), "seed {seed} played {cell:?}");
    }
    let careless = (0..50).any(|seed| {
        let cell = think(game.state(), level(1), seed, frozen).unwrap();
        losing.contains(&cell)
    });
    assert!(careless, "Level 1 can miss it");
}

#[test]
fn from_level_three_it_always_takes_an_immediate_win() {
    // Fewer than 3 under some Rulesets: FirstTo(9) is all but unwinnable.
    let positions = sample(3, |state| (!wins(&state).is_empty()).then_some(state));
    assert!(positions.len() >= 200, "only {}", positions.len());
    for state in &positions {
        let winning = wins(state);
        for n in [3, 4] {
            for seed in 0..3 {
                let cell = think(state, level(n), seed, frozen).unwrap();
                assert!(winning.contains(&cell), "Level {n} played {cell:?}");
            }
        }
    }
}

#[test]
fn from_level_three_it_never_hands_over_a_win_it_could_avoid() {
    let positions = sample(3, avoidable_losses);
    assert!(positions.len() >= 200, "only {}", positions.len());
    let mut missed_at_level_one = 0;
    for (state, losing) in &positions {
        avoids(state, losing);
        let cell = think(state, level(1), 0, frozen).unwrap();
        missed_at_level_one += usize::from(losing.contains(&cell));
    }
    assert!(
        missed_at_level_one > 0,
        "the positions are hard enough to miss"
    );
}

#[test]
fn from_level_three_it_never_ends_the_game_in_its_own_loss() {
    // A move can lose at once: deciding the last Board while behind on count, or, when drawn
    // Boards count for both, drawing one that completes the opponent's line.
    let positions = sample(1, |state| {
        let suicidal = ending_with(&state, state.ply().player().opponent());
        if suicidal.is_empty() {
            return None;
        }
        avoidable_losses(state).map(|_| (state, suicidal))
    });
    assert!(positions.len() >= 20, "only {}", positions.len());
    for (state, suicidal) in &positions {
        avoids(state, suicidal);
    }
}
