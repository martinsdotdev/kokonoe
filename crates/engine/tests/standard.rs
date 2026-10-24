//! Standard at depth 2, through the public API.

use kokonoe_engine::{
    Board, BoardStatus, Cell, Child, Depth, Game, Goal, IllegalMove, Outcome, ParseError,
    PlayError, Played, Player, Ply, Preset, RandomPlies, Seed, Sent, Space, Status,
};

/// Local board `n` of a depth-2 game.
fn local(n: usize) -> Board {
    match Board::global(Depth::Two).child(Space::ALL[n]) {
        Child::Board(board) => board,
        Child::Cell(_) => unreachable!(),
    }
}

/// Space-separated Cells, each written as its two digits: `"42 24"` is `4·2` then `2·4`.
fn cells(text: &str) -> Vec<Cell> {
    text.split_whitespace()
        .map(|digits| {
            let digits: Vec<u8> = digits.bytes().map(|b| b - b'0').collect();
            Cell::from_digits(&digits).unwrap()
        })
        .collect()
}

fn cell(digits: &str) -> Cell {
    cells(digits)[0]
}

/// A Standard game with `moves` played in order.
fn standard(moves: &str) -> Game {
    let mut game = Game::new(Preset::Standard.ruleset(), Seed(0));
    for cell in cells(moves) {
        let at = game.state().ply();
        game.play(at, cell).unwrap();
    }
    game
}

/// X takes Local boards 0, 1 and 2, each with a row, while O sends X wherever it needs to go.
const TOP_ROW: &str = "03 30 04 40 05 51 13 31 14 41 15 52 26 62 27 72 28";

/// The shortest drawn game among 200,000 random ones.
const DRAWN: &str = "82 21 12 27 72 24 46 64 42 78 85 54 44 11 16 68 87 70 03 32 65 56 \
                     62 01 14 67 77 71 38 83 36 63 34 07 76 66 84 04 37 74 80 52 81";

fn to_move(game: &Game) -> (Player, Sent, Vec<Cell>) {
    match game.state().status() {
        Status::ToMove {
            player,
            sent,
            moves,
        } => (player, sent, moves.iter().collect()),
        Status::Over(outcome) => panic!("the game is over: {outcome:?}"),
    }
}

/// The empty Cells of a Local board, in address order.
fn empty_cells(game: &Game, board: Board) -> Vec<Cell> {
    Space::ALL
        .into_iter()
        .filter_map(|space| match board.child(space) {
            Child::Cell(cell) => game.state().mark(cell).is_none().then_some(cell),
            Child::Board(_) => unreachable!(),
        })
        .collect()
}

#[test]
fn the_first_move_may_go_anywhere() {
    let game = standard("");
    let (player, sent, moves) = to_move(&game);
    assert_eq!(
        (player, sent, moves.len()),
        (Player::X, Sent::FirstMove, 81)
    );
}

#[test]
fn a_move_sends_the_opponent_to_the_local_board_of_its_space() {
    let game = standard("42");
    let (player, sent, moves) = to_move(&game);
    assert_eq!((player, sent), (Player::O, Sent::To(local(2))));
    assert_eq!(moves, empty_cells(&game, local(2)));

    let game = standard("42 24");
    let (player, sent, moves) = to_move(&game);
    assert_eq!((player, sent), (Player::X, Sent::To(local(4))));
    assert_eq!(moves.len(), 8, "4·2 is taken");
    assert_eq!(moves, empty_cells(&game, local(4)));
}

#[test]
fn a_decided_sent_target_gives_a_free_move() {
    // X wins Local board 0 with 0·5, then O sends X back to it.
    let game = standard("03 30 04 40 05 50");
    assert_eq!(
        game.state().board_status(local(0)),
        BoardStatus::Won(Player::X)
    );
    let (player, sent, moves) = to_move(&game);
    assert_eq!(player, Player::X);
    assert_eq!(
        sent,
        Sent::Free {
            target: local(0),
            within: Board::global(Depth::Two),
        }
    );
    let open: Vec<Cell> = (1..9).flat_map(|n| empty_cells(&game, local(n))).collect();
    assert_eq!(moves, open);
    assert_eq!(
        moves.len(),
        72 - 3,
        "every empty Cell outside Local board 0"
    );
}

#[test]
fn three_won_local_boards_in_a_row_win_the_game() {
    let mut game = standard(TOP_ROW);
    assert_eq!(
        game.state().status(),
        Status::Over(Outcome::Objective(Player::X))
    );
    assert_eq!(
        game.state().board_status(Board::global(Depth::Two)),
        BoardStatus::Won(Player::X)
    );
    assert_eq!(game.state().won_count(Player::X), 3);
    assert_eq!(game.state().won_count(Player::O), 0);

    let at = game.state().ply();
    assert_eq!(
        game.play(at, cell("88")),
        Err(PlayError::Illegal(IllegalMove::GameOver))
    );
}

#[test]
fn every_local_board_decided_with_no_line_is_a_draw() {
    let game = standard(DRAWN);
    assert_eq!(game.state().status(), Status::Over(Outcome::Draw));
    for n in 0..9 {
        assert!(
            game.state().board_status(local(n)).is_decided(),
            "Local board {n}"
        );
    }
    assert_eq!(
        game.state().board_status(Board::global(Depth::Two)),
        BoardStatus::Drawn
    );
}

#[test]
fn ply_stamps_make_resends_harmless() {
    let mut game = standard("42 24");
    let (record, state) = (game.record(), *game.state());

    assert_eq!(game.play(Ply::new(1), cell("24")), Ok(Played::Already));
    assert_eq!(game.play(Ply::new(0), cell("42")), Ok(Played::Already));
    assert_eq!(
        game.play(Ply::new(1), cell("25")),
        Err(PlayError::OutOfOrder {
            want: Ply::new(2),
            got: Ply::new(1),
        })
    );
    assert_eq!(
        game.play(Ply::new(3), cell("40")),
        Err(PlayError::OutOfOrder {
            want: Ply::new(2),
            got: Ply::new(3),
        })
    );
    assert_eq!(game.record(), record);
    assert!(*game.state() == state, "nothing changed");

    assert!(matches!(
        game.play(Ply::new(2), cell("40")),
        Ok(Played::Now(_))
    ));
}

#[test]
fn an_illegal_move_says_why_and_changes_nothing() {
    let mut game = standard("44");
    let state = *game.state();
    let at = game.state().ply();
    let mut illegal = |digits: &str| match game.play(at, cell(digits)) {
        Err(PlayError::Illegal(why)) => why,
        other => panic!("{digits} was not illegal: {other:?}"),
    };
    assert_eq!(illegal("44"), IllegalMove::Occupied);
    assert_eq!(
        illegal("50"),
        IllegalMove::OutsideActiveBoards { within: local(4) }
    );
    assert_eq!(illegal("444"), IllegalMove::WrongDepth);
    assert!(*game.state() == state);

    // Local board 0 is X's, and O is sent to Local board 5.
    let mut game = standard("03 30 04 40 05");
    let at = game.state().ply();
    assert_eq!(
        game.play(at, cell("00")),
        Err(PlayError::Illegal(IllegalMove::BeneathDecided {
            board: local(0)
        }))
    );
}

#[test]
fn undo_and_state_at_replay_the_history() {
    let mut game = standard("42 24 40");
    let after_two = *standard("42 24").state();
    assert!(game.state_at(Ply::new(2)) == Some(after_two));
    assert!(game.state_at(Ply::START) == Some(*standard("").state()));
    assert!(game.state_at(Ply::new(4)).is_none());

    assert_eq!(game.undo(), Some(cell("40")));
    assert!(*game.state() == after_two);
    assert_eq!(game.moves(), cells("42 24"));
    game.undo();
    game.undo();
    assert_eq!(game.undo(), None);
}

#[test]
fn open_replays_a_record_and_stops_at_the_first_illegal_move() {
    let game = standard(DRAWN);
    let opened = Game::open(&game.record()).unwrap();
    assert!(opened.state() == game.state());

    let mut record = standard("42 24").record();
    record.moves.push(cell("24"));
    let error = Game::open(&record).err().unwrap();
    assert_eq!((error.index, error.error), (2, IllegalMove::Occupied));
}

#[test]
fn addresses_parse_at_the_boundary() {
    assert_eq!(Cell::from_digits(&[4]), Err(ParseError::AddressLength(1)));
    assert_eq!(
        Cell::from_digits(&[0, 0, 0, 0]),
        Err(ParseError::AddressLength(4))
    );
    assert_eq!(Cell::from_digits(&[4, 9]), Err(ParseError::Space(9)));

    let c = cell("42");
    assert_eq!(c.depth(), Depth::Two);
    assert_eq!(&*c.spaces(), &[Space::ALL[4], Space::ALL[2]]);
    assert_eq!(c.local_board(), local(4));
    assert_eq!(c.sends_to(), local(2));
}

#[test]
fn boards_and_cells_round_trip_through_parent_and_child() {
    let global = Board::global(Depth::Two);
    assert_eq!(global.parent(), None);
    assert!(global.spaces().is_empty());
    for space in Space::ALL {
        let board = local(usize::from(space.index()));
        assert!(board.is_local());
        assert_eq!(board.parent(), Some(global));
        assert_eq!(&*board.spaces(), &[space]);
        for inner in Space::ALL {
            let Child::Cell(c) = board.child(inner) else {
                unreachable!()
            };
            assert_eq!(c.local_board(), board);
            assert_eq!(&*c.spaces(), &[space, inner]);
        }
    }
}

#[test]
fn rule_settings_parse_at_the_boundary() {
    assert_eq!(Depth::try_from(4), Err(ParseError::Depth(4)));
    assert_eq!(Depth::try_from(3), Ok(Depth::Three));
    assert_eq!(Goal::try_from(0), Err(ParseError::Goal(0)));
    assert_eq!(Goal::try_from(10), Err(ParseError::Goal(10)));
    assert_eq!(Goal::try_from(9).map(Goal::get), Ok(9));
    assert_eq!(RandomPlies::try_from(0), Err(ParseError::RandomPlies(0)));
    assert_eq!(RandomPlies::try_from(9), Err(ParseError::RandomPlies(9)));
    for preset in Preset::ALL {
        assert_eq!(
            Preset::of(&preset.ruleset()),
            Some(preset),
            "{}",
            preset.name()
        );
    }
}
