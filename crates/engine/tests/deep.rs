//! Depth 3 (spec §2.1) through the public API: Sent drops a move's outermost step, relaxation
//! walks up through the middle Boards, and Continuation and the objective work a level up. The
//! property test checks every ply against a reference model; these pin the rules by name.

mod common;

use kokonoe_engine::{
    Board, BoardStatus, Cell, Continuation, Depth, Game, Opening, Outcome, Player, Ply, Preset,
    RandomPlies, Ruleset, Seed, Sent, Status,
};

use common::{Xorshift, advance, cells, game};

const DEEP: Ruleset = Preset::Deep.ruleset();

fn board(path: &[u8]) -> Board {
    Board::from_path(Depth::Three, path).unwrap()
}

/// The Board one level up: a Local board's middle Board.
fn parent(board: Board) -> Board {
    board.parent().unwrap()
}

#[test]
fn sent_drops_the_outermost_step() {
    let first = cells("407")[0];
    assert_eq!(first.to_string(), "5·1·8");
    let game = game(DEEP, "407");
    let Status::ToMove { sent, moves, .. } = game.state().status() else {
        panic!("the game goes on");
    };
    assert_eq!(sent, Sent::To(board(&[0, 7])), "Local board 1·8");
    let local_board: Vec<Cell> = (0..9)
        .map(|space| Cell::from_digits(&[0, 7, space]).unwrap())
        .collect();
    assert_eq!(moves.iter().collect::<Vec<_>>(), local_board);
}

/// Found by seeded random search as the shortest Deep game to need it. X takes the middle row of
/// Local board 7·6 (8·7 in Notation) with 7·6·3, 7·6·4 and 7·6·5, then O's 0·7·6 Sends X there.
const INTO_A_WON_BOARD: &str = "257 576 765 653 537 376 763 636 367 676 764 640 407 076";

#[test]
fn relaxation_stops_at_the_middle_board() {
    let game = game(DEEP, INTO_A_WON_BOARD);
    let target = board(&[7, 6]);
    assert_eq!(
        game.state().board_status(target),
        BoardStatus::Won(Player::X)
    );
    assert_eq!(game.state().board_status(parent(target)), BoardStatus::Open);

    let Status::ToMove { sent, moves, .. } = game.state().status() else {
        panic!("the game goes on");
    };
    // With Continuation off the won Local board takes no move, so X may play anywhere legal in
    // the middle Board above it: its 81 Cells, less the won board's 9.
    let within = parent(target);
    assert_eq!(sent, Sent::Free { target, within });
    assert_eq!(moves.len(), 72);
    assert!(moves.iter().all(|cell| cell.spaces()[0].index() == 7));
    assert!(moves.iter().all(|cell| cell.local_board() != target));
}

/// Plays a random game under `rules` from `seed`, calling `each` with the game after every move.
fn random_game(rules: Ruleset, seed: u64, mut each: impl FnMut(&Game)) {
    let mut game = Game::new(rules, Seed(seed));
    let mut random = Xorshift(seed);
    while let Status::ToMove { moves, .. } = game.state().status() {
        let at = game.state().ply();
        game.play(at, moves.nth(random.below(moves.len()))).unwrap();
        each(&game);
    }
}

#[test]
fn relaxation_reaches_the_global_board_only_past_a_decided_middle_board() {
    let (mut middle, mut global) = (0, 0);
    for seed in 1..=60 {
        random_game(DEEP, seed, |game| {
            let state = game.state();
            let Status::ToMove {
                sent: Sent::Free { target, within },
                moves,
                ..
            } = state.status()
            else {
                return;
            };
            // The target takes no move: it's decided, or it lies in a decided middle Board.
            let decided = |board| state.board_status(board).is_decided();
            assert!(decided(target) || decided(parent(target)));
            if within == parent(target) {
                middle += 1;
                assert!(decided(target) && !decided(within));
            } else {
                global += 1;
                assert_eq!(within, Board::global(Depth::Three));
                assert!(decided(parent(target)));
                // Anywhere legal: no Cell beneath a decided Board, at either level.
                assert!(moves.iter().all(|cell| {
                    !decided(cell.local_board()) && !decided(parent(cell.local_board()))
                }));
            }
        });
    }
    assert!(middle > 0 && global > 0, "{middle} middle, {global} global");
}

#[test]
fn continuation_keeps_a_won_middle_board_playable_and_its_owner_fixed() {
    let rules = Ruleset {
        continuation: Continuation::On,
        ..DEEP
    };
    let middles: Vec<Board> = (0..9).map(|space| board(&[space])).collect();
    let mut inside = 0;
    for seed in 1..=40 {
        let mut owners: Vec<Option<Player>> = vec![None; 9];
        random_game(rules, seed, |game| {
            let state = game.state();
            let last = game.moves().last().unwrap().spaces()[0].index();
            if owners[usize::from(last)].is_some() {
                inside += 1;
            }
            for (owner, &middle) in owners.iter_mut().zip(&middles) {
                if let BoardStatus::Won(player) = state.board_status(middle) {
                    assert_eq!(
                        *owner.get_or_insert(player),
                        player,
                        "an owner never changes"
                    );
                }
            }
        });
    }
    assert!(inside > 0, "moves land inside won middle Boards");
}

#[test]
fn a_line_of_middle_boards_wins() {
    const LINES: [[u8; 3]; 8] = [
        [0, 1, 2],
        [3, 4, 5],
        [6, 7, 8],
        [0, 3, 6],
        [1, 4, 7],
        [2, 5, 8],
        [0, 4, 8],
        [2, 4, 6],
    ];
    let mut wins = 0;
    for seed in 1..=60 {
        random_game(DEEP, seed, |game| {
            let state = game.state();
            let Status::Over(Outcome::Objective(winner)) = state.status() else {
                return;
            };
            wins += 1;
            let won = |space: u8| state.board_status(board(&[space])) == BoardStatus::Won(winner);
            assert!(
                LINES
                    .iter()
                    .any(|line| line.iter().all(|&space| won(space)))
            );
        });
    }
    assert!(wins > 0, "some random games end on a line");
}

/// Found by a guided search, and played by tap in `web/src/lib/game/games.fixtures.ts`, which mirrors it: X
/// wins the top row of middle Boards in 53 plies, the fewest it can. X wins every Local board on
/// its top row, so its Cell's last Space Sends O where O's reply Sends X back into middle Board 1,
/// 2 or 3; O never wins a Board. On the way O Sends X into a Local board X has won, a Free move
/// inside a middle Board, and into a won middle Board, a Free move anywhere: five of the one and
/// two of the other, which the phone test counts in taps.
const LINE_OF_MIDDLE_BOARDS: &str = "280 803 031 313 131 315 151 515 150 504 042 427 271 714 142 \
    426 262 627 272 728 281 815 152 527 270 703 032 327 261 615 130 304 041 413 132 328 282 827 \
    260 604 040 403 030 305 052 522 050 503 051 513 140 405 141";

#[test]
fn a_short_game_wins_on_a_line_of_middle_boards() {
    let mut game = Game::new(DEEP, Seed(0));
    let (mut middle, mut global) = (0, 0);
    for cell in cells(LINE_OF_MIDDLE_BOARDS) {
        advance(&mut game, cell);
        match game.state().status() {
            Status::ToMove {
                sent: Sent::Free { within, .. },
                ..
            } if within == Board::global(Depth::Three) => global += 1,
            Status::ToMove {
                sent: Sent::Free { .. },
                ..
            } => middle += 1,
            _ => {}
        }
    }
    let state = game.state();
    assert_eq!(game.moves().len(), 53);
    assert_eq!(state.status(), Status::Over(Outcome::Objective(Player::X)));
    for space in [0, 1, 2] {
        assert_eq!(
            state.board_status(board(&[space])),
            BoardStatus::Won(Player::X)
        );
    }
    assert_eq!(
        (middle, global),
        (5, 2),
        "Free moves inside a middle Board, and anywhere"
    );
}

/// The Seed whose two-ply Deep deal, X on 702 then O on 028 (8·1·3 and 1·3·9 in Notation), Sends
/// X to Local board 2·8, where `LINE_OF_MIDDLE_BOARDS` starts, and leaves the game to end as
/// before. Found by searching Seeds
/// from 0; `web/src/lib/game/games.fixtures.ts` mirrors it as `DEALT`, which the Replay viewer's
/// tests replay.
const DEALT_SEED: Seed = Seed(22);

#[test]
fn a_short_game_after_a_dealt_opening_replays_ply_by_ply() {
    let rules = Ruleset {
        opening: Opening::Random(RandomPlies::try_from(2).unwrap()),
        ..DEEP
    };
    let mut game = Game::new(rules, DEALT_SEED);
    let opening: [Cell; 2] = cells("702 028").try_into().unwrap();
    assert_eq!(game.opening(), opening);
    for cell in cells(LINE_OF_MIDDLE_BOARDS) {
        advance(&mut game, cell);
    }
    assert_eq!(
        game.state().status(),
        Status::Over(Outcome::Objective(Player::X))
    );

    let at = |ply: u16| game.state_at(Ply::new(ply)).unwrap();
    let marks = |ply: u16| opening.map(|cell| at(ply).mark(cell));
    assert_eq!(marks(0), [None, None], "the empty board");
    assert_eq!(marks(1), [Some(Player::X), None]);
    assert_eq!(marks(2), [Some(Player::X), Some(Player::O)]);
    let end = game.state().ply();
    assert_eq!(end, Ply::new(55));
    assert!(game.state_at(end) == Some(*game.state()));
    assert!(game.state_at(Ply::new(56)).is_none());
}
