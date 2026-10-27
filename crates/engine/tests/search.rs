//! The computer opponent (spec §6): its ladder, games at every Level finishing, the time cap,
//! and, from Level 3, never missing an immediate win or handing the opponent one.

mod model;

use kokonoe_engine::{
    Cell, Choice, Depth, Game, Level, ParseError, Player, Preset, Ruleset, Seed, State, Status,
    think,
};

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

/// Space-separated Cells, each written as its two digits: `"42"` is `4·2`.
fn cells(text: &str) -> Vec<Cell> {
    text.split_whitespace()
        .map(|digits| {
            let digits: Vec<u8> = digits.bytes().map(|b| b - b'0').collect();
            Cell::from_digits(&digits).unwrap()
        })
        .collect()
}

/// Plays `cell` as the next move.
fn advance(game: &mut Game, cell: Cell) {
    let at = game.state().ply();
    game.play(at, cell).unwrap();
}

/// A game under `rules` with `moves` played in order.
fn game(rules: Ruleset, moves: &str) -> Game {
    let mut game = Game::new(rules, Seed(0));
    for cell in cells(moves) {
        advance(&mut game, cell);
    }
    game
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

/// xorshift64, for random games independent of the engine's generators.
struct TestRng(u64);

impl TestRng {
    fn below(&mut self, n: usize) -> usize {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 % n as u64) as usize
    }
}

/// Every position of 300 seeded random games under `rules`, in a fixed order.
fn positions(rules: Ruleset) -> impl Iterator<Item = State> {
    (1..=300).flat_map(move |seed| {
        let mut game = Game::new(rules, Seed(seed));
        let mut rng = TestRng(seed);
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

#[test]
fn the_ladder_is_spec_section_6() {
    let ladder: Vec<_> = (Level::MIN..=Level::MAX)
        .map(|n| {
            let level = level(n);
            (
                level.iterations(),
                level.cap_ms(Depth::Two),
                level.cap_ms(Depth::Three),
                level.choice(),
                level.tactics(),
            )
        })
        .collect();
    let sample = |tau| Choice::Sample { tau };
    assert_eq!(
        ladder,
        [
            (30, 1_000, 3_000, sample(2.0), false),
            (100, 1_000, 3_000, sample(2.0), false),
            (300, 1_000, 3_000, sample(1.0), true),
            (1_000, 1_000, 3_000, sample(1.0), true),
            (3_000, 2_000, 6_000, sample(0.5), true),
            (10_000, 2_000, 6_000, sample(0.5), true),
            (30_000, 3_000, 9_000, Choice::MostVisited, true),
            (100_000, 3_000, 9_000, Choice::MostVisited, true),
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
