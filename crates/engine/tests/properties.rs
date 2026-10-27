//! Random games under every depth-2 Ruleset, with a Free opening or Random(1) to Random(8) dealt
//! from a random Seed. Whatever the moves, the engine answers every call without panicking,
//! agrees with the reference model after every ply, dealt ones included, and replaying the
//! Record reproduces the game.

mod model;

use kokonoe_engine::{
    Board, Cell, Depth, Game, Opening, Played, Ply, RandomPlies, Ruleset, Seed, State, Status,
};
use model::{Model, depth_two_rulesets, local};
use proptest::prelude::*;

/// Every Cell of a depth-2 game, then one of a depth-3 game.
fn every_cell() -> Vec<Cell> {
    let mut cells: Vec<Cell> = (0..81)
        .map(|index| Cell::from_index(Depth::Two, index).unwrap())
        .collect();
    cells.push(Cell::from_digits(&[4, 4, 4]).unwrap());
    cells
}

/// Checks one turn against the model and against everything the engine promises about it.
/// `sweep` also offers it every Cell, which is the slow part, so a game does it once.
fn check_turn(
    state: &State,
    model: &Model,
    cells: &[Cell],
    sweep: bool,
) -> Result<(), TestCaseError> {
    if let Some(disagreement) = model.disagreement(state) {
        return Err(TestCaseError::fail(disagreement));
    }
    let Status::ToMove { moves, .. } = state.status() else {
        return Ok(());
    };
    prop_assert_eq!(state.ply().player(), model.to_move());
    // Ascending order, with `nth` agreeing: a seam invariant (`BoardRepr::legal_in`), since a
    // Random opening deals by position in this list.
    let listed: Vec<Cell> = moves.iter().collect();
    prop_assert_eq!(listed.len(), moves.len());
    prop_assert!(listed.windows(2).all(|pair| pair[0] < pair[1]));
    for (i, &cell) in listed.iter().enumerate() {
        prop_assert_eq!(moves.nth(i), cell);
    }
    for board in 0..9 {
        let inside = listed.iter().any(|cell| cell.local_board() == local(board));
        prop_assert_eq!(moves.any_in(local(board)), inside);
    }
    prop_assert!(moves.any_in(Board::global(Depth::Two)));
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

/// Plays `choices` under `rules` after the opening `seed` deals, offering every Cell at ply
/// `sweep` and at the end.
fn play_out(
    rules: Ruleset,
    seed: u64,
    choices: &[usize],
    sweep: usize,
) -> Result<(), TestCaseError> {
    let cells = every_cell();
    let mut game = Game::new(rules, Seed(seed));
    // The model never deals: it takes the dealt plies as ordinary moves.
    let mut model = Model::new(Ruleset {
        opening: Opening::Free,
        ..rules
    });
    let mut states = vec![game.state_at(Ply::START).unwrap()];
    for &cell in game.opening() {
        let state = states[states.len() - 1];
        check_turn(&state, &model, &cells, false)?;
        let legal = model.turn().map(|(_, legal)| legal).unwrap_or_default();
        let index = usize::from(cell.index());
        prop_assert!(legal.contains(&index), "dealt {:?}, not a legal move", cell);
        model.play(index);
        states.push(game.state_at(Ply::new(states.len() as u16)).unwrap());
    }
    let dealt = game.opening().len();
    match rules.opening {
        Opening::Free => prop_assert_eq!(dealt, 0),
        Opening::Random(k) => prop_assert!(
            dealt == usize::from(k.get()) || model.turn().is_none(),
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
        check_turn(&state, &model, &cells, ply == sweep)?;
        let Status::ToMove { moves, .. } = state.status() else {
            break;
        };
        let cell = moves.nth(choice);
        let played = game.play(state.ply(), cell);
        prop_assert!(matches!(played, Ok(Played::Now(_))));
        model.play(usize::from(cell.index()));
        states.push(*game.state());
    }
    check_turn(game.state(), &model, &cells, true)?;
    prop_assert!(game.state().ply().get() <= 81);

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
