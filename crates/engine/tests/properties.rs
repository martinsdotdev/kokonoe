//! Random Standard games. Whatever the moves, the engine answers every call without panicking,
//! every turn has moves, and replaying the Record reproduces the game.

use kokonoe_engine::{
    Board, Cell, Child, Depth, Game, Played, Player, Ply, Preset, Seed, Sent, Space, State, Status,
};
use proptest::prelude::*;

/// Local board `n` of a depth-2 game.
fn local(n: u8) -> Board {
    match Board::global(Depth::Two).child(Space::try_from(n).unwrap()) {
        Child::Board(board) => board,
        Child::Cell(_) => unreachable!(),
    }
}

/// Every Cell of a depth-2 game, then one of a depth-3 game.
fn every_cell() -> Vec<Cell> {
    let mut cells: Vec<Cell> = (0..9)
        .flat_map(|board| (0..9).map(move |space| Cell::from_digits(&[board, space]).unwrap()))
        .collect();
    cells.push(Cell::from_digits(&[4, 4, 4]).unwrap());
    cells
}

/// Checks one turn against everything the engine promises about it. `last` is the move that led
/// here.
fn check_turn(state: &State, last: Option<Cell>, cells: &[Cell]) -> Result<(), TestCaseError> {
    let global = Board::global(Depth::Two);
    state.board_status(global);
    for n in 0..9 {
        state.board_status(local(n));
    }
    for &cell in cells {
        state.mark(cell);
    }
    state.won_count(Player::X);
    state.won_count(Player::O);

    let Status::ToMove {
        player,
        sent,
        moves,
    } = state.status()
    else {
        return Ok(());
    };
    prop_assert_eq!(player, state.ply().player());
    prop_assert!(moves.len() >= 1);
    let listed: Vec<Cell> = moves.iter().collect();
    prop_assert_eq!(listed.len(), moves.len());
    prop_assert!(listed.windows(2).all(|pair| pair[0] < pair[1]));
    for (i, &cell) in listed.iter().enumerate() {
        prop_assert_eq!(moves.nth(i), cell);
    }
    for n in 0..9 {
        let inside = listed.iter().any(|cell| cell.local_board() == local(n));
        prop_assert_eq!(moves.any_in(local(n)), inside);
    }
    // Sent, read straight off the digits: `b·s` sends to Local board `s`.
    let sent_to = last.map(|cell| local(cell.spaces()[1].index()));
    match sent {
        Sent::FirstMove => {
            prop_assert_eq!(last, None);
            prop_assert_eq!(moves.len(), 81);
        }
        Sent::To(target) => {
            prop_assert_eq!(Some(target), sent_to);
            prop_assert!(!state.board_status(target).is_decided());
            prop_assert!(listed.iter().all(|cell| cell.local_board() == target));
        }
        Sent::Free { target, within } => {
            prop_assert_eq!(Some(target), sent_to);
            prop_assert_eq!(within, global);
            prop_assert!(state.board_status(target).is_decided());
            for n in 0..9 {
                let open = !state.board_status(local(n)).is_decided();
                prop_assert_eq!(moves.any_in(local(n)), open);
            }
        }
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
    #[test]
    fn random_standard_games_hold_together(choices in prop::collection::vec(any::<usize>(), 0..100)) {
        let cells = every_cell();
        let mut game = Game::new(Preset::Standard.ruleset(), Seed(0));
        let mut states = vec![*game.state()];
        let mut last = None;
        for choice in choices {
            let state = *game.state();
            check_turn(&state, last, &cells)?;
            let Status::ToMove { moves, .. } = state.status() else {
                break;
            };
            let cell = moves.nth(choice);
            let played = game.play(state.ply(), cell);
            prop_assert!(matches!(played, Ok(Played::Now(_))));
            states.push(*game.state());
            last = Some(cell);
        }
        check_turn(game.state(), last, &cells)?;
        prop_assert!(game.state().ply().get() <= 81);

        let opened = Game::open(&game.record()).unwrap();
        prop_assert!(opened.state() == game.state(), "open reproduces the State");
        for (ply, state) in (0u16..).zip(&states) {
            prop_assert!(game.state_at(Ply::new(ply)) == Some(*state), "state_at({})", ply);
        }
        if let [.., before, _] = states[..] {
            game.undo();
            prop_assert!(*game.state() == before, "undo gives back the previous State");
        }
    }
}
