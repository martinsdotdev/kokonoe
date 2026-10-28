//! Random games under every Ruleset, at depth 2 and depth 3, with a Free opening or Random(1) to
//! Random(8) dealt from a random Seed. Whatever the moves, the engine answers every call without
//! panicking, agrees with the reference model of its depth after every ply, dealt ones included,
//! and replaying the Record reproduces the game.

mod common;
mod model;

use common::Xorshift;
use kokonoe_engine::{
    Board, Cell, Child, Depth, Game, Opening, Played, Ply, RandomPlies, Ruleset, Seed, Space,
    State, Status,
};
use model::deep::DeepModel;
use model::{Model, depth_three_rulesets, depth_two_rulesets};
use proptest::prelude::*;

/// What the test asks of a reference model, at either depth.
trait Reference {
    /// The legal Cells by index, in address order; empty once the game is over.
    fn legal(&self) -> Vec<usize>;
    fn play(&mut self, cell: usize);
    fn disagreement(&self, state: &State) -> Option<String>;
}

impl Reference for Model {
    fn legal(&self) -> Vec<usize> {
        self.turn().map(|(_, legal)| legal).unwrap_or_default()
    }
    fn play(&mut self, cell: usize) {
        Model::play(self, cell);
    }
    fn disagreement(&self, state: &State) -> Option<String> {
        Model::disagreement(self, state)
    }
}

impl Reference for DeepModel {
    fn legal(&self) -> Vec<usize> {
        self.turn().map(|(_, legal)| legal).unwrap_or_default()
    }
    fn play(&mut self, cell: usize) {
        DeepModel::play(self, cell);
    }
    fn disagreement(&self, state: &State) -> Option<String> {
        DeepModel::disagreement(self, state)
    }
}

/// The model of `rules`' depth. It never deals: it takes the dealt plies as ordinary moves.
fn reference(rules: Ruleset) -> Box<dyn Reference> {
    let rules = Ruleset {
        opening: Opening::Free,
        ..rules
    };
    match rules.depth {
        Depth::Two => Box::new(Model::new(rules)),
        Depth::Three => Box::new(DeepModel::new(rules)),
    }
}

/// Every Cell of a game at `depth`, then one of the other depth.
fn every_cell(depth: Depth) -> Vec<Cell> {
    let (count, other) = match depth {
        Depth::Two => (81, Cell::from_digits(&[4, 4, 4])),
        Depth::Three => (729, Cell::from_digits(&[4, 4])),
    };
    let mut cells: Vec<Cell> = (0..count)
        .map(|index| Cell::from_index(depth, index).unwrap())
        .collect();
    cells.push(other.unwrap());
    cells
}

/// Every Board below the Global board at `depth`: the Local boards, and at depth 3 the middle
/// Boards above them.
fn every_board(depth: Depth) -> Vec<Board> {
    let mut boards = Vec::new();
    let mut level = vec![Board::global(depth)];
    while let Some(Child::Board(_)) = level.first().map(|board| board.child(Space::ALL[0])) {
        level = level
            .iter()
            .flat_map(|board| Space::ALL.map(|space| board.child(space)))
            .filter_map(|child| match child {
                Child::Board(board) => Some(board),
                Child::Cell(_) => None,
            })
            .collect();
        boards.extend(&level);
    }
    boards
}

/// Checks one turn against the model and against everything the engine promises about it.
/// `sweep` also offers it every Cell, which is the slow part, so a game does it once.
fn check_turn(
    state: &State,
    model: &dyn Reference,
    cells: &[Cell],
    boards: &[Board],
    sweep: bool,
) -> Result<(), TestCaseError> {
    if let Some(disagreement) = model.disagreement(state) {
        return Err(TestCaseError::fail(disagreement));
    }
    let Status::ToMove { moves, .. } = state.status() else {
        return Ok(());
    };
    // Ascending order, with `nth` agreeing: a seam invariant (`BoardRepr::legal_in`), since a
    // Random opening deals by position in this list.
    let listed: Vec<Cell> = moves.iter().collect();
    prop_assert_eq!(listed.len(), moves.len());
    prop_assert!(listed.windows(2).all(|pair| pair[0] < pair[1]));
    for (i, &cell) in listed.iter().enumerate() {
        prop_assert_eq!(moves.nth(i), cell);
    }
    for &board in boards {
        let inside = listed
            .iter()
            .any(|cell| cell.spaces().starts_with(&board.spaces()));
        prop_assert_eq!(moves.any_in(board), inside, "{:?}", board);
    }
    prop_assert!(moves.any_in(Board::global(state.ruleset().depth)));
    if !sweep {
        return Ok(());
    }
    for &cell in cells {
        let mut copy = *state;
        let played = copy.play(cell);
        prop_assert_eq!(played.is_ok(), moves.contains(cell), "{:?}", cell);
        if played.is_err() {
            prop_assert!(copy == *state, "a refused move changed the State");
        }
    }
    Ok(())
}

/// Free, then Random(1) to Random(8).
fn openings() -> Vec<Opening> {
    let random = (1..=8).map(|k| Opening::Random(RandomPlies::try_from(k).unwrap()));
    [Opening::Free].into_iter().chain(random).collect()
}

proptest! {
    // About 25 games under each of the 80 Rulesets and 230 under each opening, so missing one
    // is vanishingly unlikely.
    #![proptest_config(ProptestConfig::with_cases(2048))]

    #[test]
    fn random_games_hold_together(
        rules in prop::sample::select(depth_two_rulesets()),
        opening in prop::sample::select(openings()),
        seed in any::<u64>(),
        choices in prop::collection::vec(any::<usize>(), 0..100),
        sweep in 0..82usize,
    ) {
        play_out(Ruleset { opening, ..rules }, seed, &choices, sweep)?;
    }
}

proptest! {
    // Depth-3 games run to hundreds of plies, so fewer of them: about 3 under each Ruleset.
    // `every_depth_three_ruleset_plays_like_the_model` covers each one for certain.
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn deep_random_games_hold_together(
        rules in prop::sample::select(depth_three_rulesets()),
        opening in prop::sample::select(openings()),
        seed in any::<u64>(),
        choices in prop::collection::vec(any::<usize>(), 0..800),
        sweep in 0..800usize,
    ) {
        play_out(Ruleset { opening, ..rules }, seed, &choices, sweep)?;
    }
}

/// One seeded game under each of the 80 depth-3 Rulesets, played to its end.
#[test]
fn every_depth_three_ruleset_plays_like_the_model() {
    let mut rng = Xorshift(0x5eed);
    let mut next = || rng.next() as usize;
    for rules in depth_three_rulesets() {
        let choices: Vec<usize> = (0..729).map(|_| next()).collect();
        let sweep = next() % 300;
        if let Err(failure) = play_out(rules, next() as u64, &choices, sweep) {
            panic!("{rules:?}: {failure}");
        }
    }
}

/// Plays `choices` under `rules` after the opening `seed` deals, offering every Cell at ply
/// `sweep` and at the end.
fn play_out(
    rules: Ruleset,
    seed: u64,
    choices: &[usize],
    sweep: usize,
) -> Result<(), TestCaseError> {
    let depth = rules.depth;
    let (cells, boards) = (every_cell(depth), every_board(depth));
    let mut game = Game::new(rules, Seed(seed));
    let mut model = reference(rules);
    let mut states = vec![game.state_at(Ply::START).unwrap()];
    for &cell in game.opening() {
        let state = states[states.len() - 1];
        check_turn(&state, model.as_ref(), &cells, &boards, false)?;
        let index = usize::from(cell.index());
        prop_assert!(
            model.legal().contains(&index),
            "dealt {:?}, not a legal move",
            cell
        );
        model.play(index);
        states.push(game.state_at(Ply::new(states.len() as u16)).unwrap());
    }
    let dealt = game.opening().len();
    match rules.opening {
        Opening::Free => prop_assert_eq!(dealt, 0),
        Opening::Random(k) => prop_assert!(
            dealt == usize::from(k.get()) || model.legal().is_empty(),
            "dealt {} of {} with the game still on",
            dealt,
            k.get()
        ),
    }
    prop_assert!(
        states[dealt] == *game.state(),
        "the deal leaves the game where it says"
    );
    for (ply, &choice) in choices.iter().enumerate() {
        let state = *game.state();
        check_turn(&state, model.as_ref(), &cells, &boards, ply == sweep)?;
        let Status::ToMove { moves, .. } = state.status() else {
            break;
        };
        let cell = moves.nth(choice);
        let played = game.play(state.ply(), cell);
        prop_assert!(matches!(played, Ok(Played::Now(_))));
        model.play(usize::from(cell.index()));
        states.push(*game.state());
    }
    check_turn(game.state(), model.as_ref(), &cells, &boards, true)?;
    prop_assert!(usize::from(game.state().ply().get()) < cells.len());

    let opened = Game::open(&game.record()).unwrap();
    prop_assert!(opened.state() == game.state(), "open reproduces the State");
    for (ply, state) in (0u16..).zip(&states) {
        prop_assert!(
            game.state_at(Ply::new(ply)) == Some(*state),
            "state_at({})",
            ply
        );
    }
    if game.moves().is_empty() {
        let after = *game.state();
        prop_assert!(game.undo().is_none(), "a dealt ply can't be taken back");
        prop_assert!(*game.state() == after);
    } else if let [.., before, _] = states[..] {
        game.undo();
        prop_assert!(
            *game.state() == before,
            "undo gives back the previous State"
        );
    }
    Ok(())
}

/// Plays out `rules` from `seed`'s game with `State::playout`, then replays the same picks
/// through `play` and `LegalMoves::nth`: the counts offered, the end and the whole State must
/// match, since the playout is only a faster way to make the same moves.
fn plays_out_like_play_and_nth(rules: Ruleset, seed: u64) -> Result<(), TestCaseError> {
    let game = Game::new(rules, Seed(seed));
    let mut fast = *game.state();
    let mut picks = Vec::new();
    let mut rng = Xorshift(seed | 1);
    let outcome = fast.playout(|n| {
        let k = rng.below(n);
        picks.push((n, k));
        k
    });
    let mut slow = *game.state();
    for (n, k) in picks {
        let Status::ToMove { moves, .. } = slow.status() else {
            return Err(TestCaseError::fail(
                "the playout moved after the game ended",
            ));
        };
        prop_assert_eq!(moves.len(), n);
        slow.play(moves.nth(k)).unwrap();
    }
    prop_assert_eq!(slow.status(), Status::Over(outcome));
    prop_assert!(slow == fast, "the playout leaves the State play would");
    Ok(())
}

/// Every Ruleset at both depths, each with a different opening drawn from its seed.
#[test]
fn every_ruleset_plays_out_like_play_and_nth() {
    let openings = openings();
    let rulesets = [depth_two_rulesets(), depth_three_rulesets()].concat();
    for (seed, rules) in (1u64..).zip(rulesets) {
        let opening = openings[seed as usize % openings.len()];
        let rules = Ruleset { opening, ..rules };
        if let Err(failure) = plays_out_like_play_and_nth(rules, seed) {
            panic!("{rules:?}: {failure}");
        }
    }
}
