//! Random games under every depth-2 Ruleset. Whatever the moves, the engine answers every call
//! without panicking, agrees with the reference model after every ply, and replaying the Record
//! reproduces the game.

mod model;

use kokonoe_engine::{Board, Cell, Depth, Game, Played, Ply, Ruleset, Seed, State, Status};
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

proptest! {
    // About 25 games under each of the 80 Rulesets, so missing one is vanishingly unlikely.
    #![proptest_config(ProptestConfig::with_cases(2048))]

    #[test]
    fn random_games_hold_together(
        rules in prop::sample::select(depth_two_rulesets()),
        choices in prop::collection::vec(any::<usize>(), 0..100),
        sweep in 0..82usize,
    ) {
        play_out(rules, &choices, sweep)?;
    }
}

/// Plays `choices` under `rules`, offering every Cell at ply `sweep` and at the end.
fn play_out(rules: Ruleset, choices: &[usize], sweep: usize) -> Result<(), TestCaseError> {
    let cells = every_cell();
    let mut game = Game::new(rules, Seed(0));
    let mut model = Model::new(rules);
    let mut states = vec![*game.state()];
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
    if let [.., before, _] = states[..] {
        game.undo();
        prop_assert!(
            *game.state() == before,
            "undo gives back the previous State"
        );
    }
    Ok(())
}
