//! The four rule settings beyond Standard at depth 2: Continuation, drawn-board owner Both,
//! FirstTo(N) and the MostWon fallback. Each test plays a game found by seeded random search as
//! the shortest that shows its rule, and checks the position it relies on before the rule itself.

mod common;
mod model;

use common::game;
use kokonoe_engine::{
    Board, BoardStatus, Cell, Continuation, Depth, DrawnOwner, Fallback, Game, Goal, IllegalMove,
    Objective, Outcome, Player, Preset, Ruleset, Seed, Sent, State, Status,
};
use model::{Model, local};

const STANDARD: Ruleset = Preset::Standard.ruleset();

fn with(edit: impl FnOnce(&mut Ruleset)) -> Ruleset {
    let mut rules = STANDARD;
    edit(&mut rules);
    rules
}

fn first_to(n: u8) -> Objective {
    Objective::FirstTo(Goal::try_from(n).unwrap())
}

/// The player who made the last move.
fn last_mover(state: &State) -> Player {
    state.ply().player().opponent()
}

/// The children of the Global board that count for `player`, a drawn one counting when `both`.
fn counts(state: &State, player: Player, both: bool) -> [bool; 9] {
    core::array::from_fn(|board| match state.board_status(local(board)) {
        BoardStatus::Won(owner) => owner == player,
        BoardStatus::Drawn => both,
        BoardStatus::Open => false,
    })
}

fn has_line(counts: [bool; 9]) -> bool {
    const LINES: [[usize; 3]; 8] = [
        [0, 1, 2],
        [3, 4, 5],
        [6, 7, 8],
        [0, 3, 6],
        [1, 4, 7],
        [2, 5, 8],
        [0, 4, 8],
        [2, 4, 6],
    ];
    LINES
        .iter()
        .any(|line| line.iter().all(|&board| counts[board]))
}

mod drawn_owner_both {
    use super::*;

    const BOTH: fn(&mut Ruleset) = |rules| rules.drawn_owner = DrawnOwner::Both;

    /// The last move, 0·6, draws Local board 0, which completes O's line of 0, 1 and 2.
    const OPPONENT_LINE: &str = "82 27 70 07 72 28 81 14 47 76 67 71 12 20 00 08 86 62 21 10 \
                                 01 18 80 03 32 26 63 36 65 50 05 51 60 02 04 42 06";

    #[test]
    fn a_drawn_board_can_complete_a_line_for_the_opponent_alone() {
        let game = game(with(BOTH), OPPONENT_LINE);
        let state = game.state();
        let mover = last_mover(state);
        assert_eq!(state.board_status(local(0)), BoardStatus::Drawn);
        assert!(has_line(counts(state, mover.opponent(), true)));
        assert!(!has_line(counts(state, mover, true)));
        assert_eq!(
            state.status(),
            Status::Over(Outcome::Objective(mover.opponent()))
        );

        let neither = game_without_the_last(STANDARD, OPPONENT_LINE);
        assert!(
            matches!(neither.status(), Status::ToMove { .. }),
            "under Neither the draw counts for nobody"
        );
    }

    /// The last move draws a Local board that completes a line for both players at once.
    const DOUBLE_LINE: &str = "42 28 81 14 41 17 71 11 60 07 70 04 40 02 24 57 77 72 27 75 \
                               50 06 62 25 54 56 61 33 37 74 22 20 82 26 23 31 32 21";

    #[test]
    fn a_double_line_goes_to_the_mover() {
        let game = game(with(BOTH), DOUBLE_LINE);
        let state = game.state();
        let mover = last_mover(state);
        assert!(has_line(counts(state, Player::X, true)));
        assert!(has_line(counts(state, Player::O, true)));
        assert_eq!(state.status(), Status::Over(Outcome::Objective(mover)));
    }

    #[test]
    fn a_drawn_board_counts_for_both_players() {
        let state = *game(with(BOTH), DOUBLE_LINE).state();
        let drawn = (0..9)
            .filter(|&board| state.board_status(local(board)) == BoardStatus::Drawn)
            .count();
        assert!(drawn > 0);
        for player in [Player::X, Player::O] {
            let won = (0..9)
                .filter(|&board| state.board_status(local(board)) == BoardStatus::Won(player))
                .count();
            assert_eq!(usize::from(state.won_count(player)), won + drawn);
        }
    }

    /// The game under `rules`, played up to but not including the last move.
    fn game_without_the_last(rules: Ruleset, moves: &str) -> State {
        let mut moves: Vec<&str> = moves.split_whitespace().collect();
        moves.pop();
        *game(rules, &moves.join(" ")).state()
    }
}

mod first_to {
    use super::*;

    #[test]
    fn first_to_one_ends_on_the_first_won_board() {
        let game = game(with(|r| r.objective = first_to(1)), "03 30 04 40 05");
        let state = game.state();
        assert_eq!(state.board_status(local(0)), BoardStatus::Won(Player::X));
        assert_eq!(state.status(), Status::Over(Outcome::Objective(Player::X)));
        assert_eq!(
            state.board_status(Board::global(Depth::Two)),
            BoardStatus::Won(Player::X)
        );
        assert_eq!(state.won_count(Player::X), 1);
    }

    /// O wins Local boards 1, 4 and 7, a column, with the game still going.
    const LINE_OF_THREE: &str = "74 41 13 35 56 64 47 77 75 57 73 31 10 01 16 67 48 84 46";

    #[test]
    fn a_line_means_nothing() {
        let state = *game(with(|r| r.objective = first_to(4)), LINE_OF_THREE).state();
        let lined = [Player::X, Player::O]
            .into_iter()
            .find(|&player| has_line(counts(&state, player, false)))
            .expect("someone has a line of won boards");
        assert_eq!(state.won_count(lined), 3);
        assert!(matches!(state.status(), Status::ToMove { .. }));

        let standard = *game(STANDARD, LINE_OF_THREE).state();
        assert_eq!(
            standard.status(),
            Status::Over(Outcome::Objective(lined)),
            "under Line the same position is won"
        );
    }

    #[test]
    fn the_count_reaching_the_goal_wins() {
        let state = *game(with(|r| r.objective = first_to(3)), LINE_OF_THREE).state();
        let lined = [Player::X, Player::O]
            .into_iter()
            .find(|&player| state.won_count(player) >= 3)
            .unwrap();
        assert_eq!(state.status(), Status::Over(Outcome::Objective(lined)));
    }
}

mod most_won {
    use super::*;

    const MOST_WON: fn(&mut Ruleset) = |rules| rules.fallback = Fallback::MostWon;

    /// Every Local board decided with no line, and the counts unequal.
    const UNEQUAL: &str = "04 47 76 67 72 23 34 44 40 07 78 80 08 84 46 64 48 88 22 27 75 \
                           51 12 26 65 57 00 54 43 30 35 17 18 14 33 11 28 61 21 24 20";

    /// Every Local board decided with no line, and the counts equal.
    const EQUAL: &str = "03 30 04 47 75 55 52 23 38 80 05 57 73 34 44 46 62 26 60 84 42 \
                         20 81 16 61 14 48 88 50 33 35 54 43 31 18 40 74 45 51 12 41 37";

    fn all_decided(state: &State) -> bool {
        (0..9).all(|board| state.board_status(local(board)).is_decided())
    }

    #[test]
    fn more_won_boards_win() {
        let state = *game(with(MOST_WON), UNEQUAL).state();
        assert!(all_decided(&state));
        let (x, o) = (state.won_count(Player::X), state.won_count(Player::O));
        assert_ne!(x, o);
        let more = if x > o { Player::X } else { Player::O };
        assert_eq!(state.status(), Status::Over(Outcome::MostWon(more)));
        assert_eq!(
            game(STANDARD, UNEQUAL).state().status(),
            Status::Over(Outcome::Draw),
            "under the Draw fallback the same game is drawn"
        );
    }

    #[test]
    fn a_tie_draws() {
        let state = *game(with(MOST_WON), EQUAL).state();
        assert!(all_decided(&state));
        assert_eq!(state.won_count(Player::X), state.won_count(Player::O));
        assert_eq!(state.status(), Status::Over(Outcome::Draw));
    }
}

mod continuation {
    use super::*;

    const ON: fn(&mut Ruleset) = |rules| rules.continuation = Continuation::On;

    /// X wins Local board 3 along its middle row with 3·3, which sends O straight back into it.
    const SENT_BACK: &str = "35 53 34 43 33";

    #[test]
    fn a_won_board_still_takes_moves() {
        let state = *game(with(ON), SENT_BACK).state();
        assert_eq!(state.board_status(local(3)), BoardStatus::Won(Player::X));
        let Status::ToMove { sent, moves, .. } = state.status() else {
            panic!("the game goes on")
        };
        assert_eq!(sent, Sent::To(local(3)));
        assert_eq!(moves.len(), 9 - 3, "every empty Cell of the won board");

        let standard = *game(STANDARD, SENT_BACK).state();
        assert!(
            matches!(
                standard.status(),
                Status::ToMove {
                    sent: Sent::Free { .. },
                    ..
                }
            ),
            "with continuation off the same send is a Free move"
        );
    }

    /// O wins Local board 4 down its middle column (4·1, 4·4, 4·7). X's last move, 4·5, then
    /// completes X's own right column (4·2, 4·5, 4·8) inside it.
    const LINE_INSIDE: &str = "64 47 74 41 14 44 48 84 42 24 45";

    #[test]
    fn ownership_never_changes() {
        let state = *game(with(ON), LINE_INSIDE).state();
        let mover = last_mover(&state);
        let inside = |space| {
            let cell = Cell::from_digits(&[4, space]).unwrap();
            state.mark(cell) == Some(mover)
        };
        assert!(
            [2, 5, 8].into_iter().all(inside),
            "the mover's line is on the board"
        );
        assert_eq!(
            state.board_status(local(4)),
            BoardStatus::Won(mover.opponent())
        );
    }

    /// The last move, 4·8, sends X to Local board 8, which X won and which is full.
    const FULL_AND_WON: &str = "88 80 08 82 28 86 68 85 58 87 77 71 18 81 17 78 83 38 84 48";

    #[test]
    fn a_full_won_board_relaxes_to_a_free_move() {
        let state = *game(with(ON), FULL_AND_WON).state();
        assert!(matches!(state.board_status(local(8)), BoardStatus::Won(_)));
        assert!(
            (0..9).all(|space| state
                .mark(Cell::from_digits(&[8, space]).unwrap())
                .is_some()),
            "Local board 8 is full"
        );
        let Status::ToMove { sent, .. } = state.status() else {
            panic!("the game goes on")
        };
        let global = Board::global(Depth::Two);
        assert_eq!(
            sent,
            Sent::Free {
                target: local(8),
                within: global
            }
        );
    }

    #[test]
    fn no_move_is_refused_as_beneath_a_decided_board() {
        let state = *game(with(ON), LINE_INSIDE).state();
        for index in 0..81 {
            let mut copy = state;
            let refused = copy
                .play(Cell::from_index(Depth::Two, index).unwrap())
                .err();
            assert!(
                !matches!(refused, Some(IllegalMove::BeneathDecided { .. })),
                "Cell {index}"
            );
        }
    }
}

#[test]
fn every_depth_two_preset_agrees_with_the_model() {
    for preset in Preset::ALL {
        let rules = preset.ruleset();
        if rules.depth != Depth::Two {
            continue;
        }
        for seed in 0..50u64 {
            let mut game = Game::new(rules, Seed(0));
            let mut model = Model::new(rules);
            let mut pick = seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1;
            while let Status::ToMove { moves, .. } = game.state().status() {
                pick ^= pick << 13;
                pick ^= pick >> 7;
                pick ^= pick << 17;
                let cell = moves.nth(usize::try_from(pick % 729).unwrap());
                let at = game.state().ply();
                game.play(at, cell).unwrap();
                model.play(usize::from(cell.index()));
                if let Some(disagreement) = model.disagreement(game.state()) {
                    panic!("{} game {seed}: {disagreement}", preset.name());
                }
            }
        }
    }
}
