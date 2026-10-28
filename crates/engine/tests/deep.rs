//! Depth 3 (spec §2.1) through the public API: Sent drops a move's outermost step, relaxation
//! walks up through the middle Boards, and Continuation and the objective work a level up. The
//! property test checks every ply against a reference model; these pin the rules by name.

mod common;

use kokonoe_engine::{
    Board, BoardStatus, Cell, Continuation, Depth, Game, Outcome, Player, Preset, Ruleset, Seed,
    Sent, Status,
};

use common::{Xorshift, cells, game};

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
