//! A reference model of the depth-3 rules (spec §2.1), as plain as the depth-2 one beside it and
//! sharing no code with the engine. Cell `i` lies on Local board `i / 9`, which lies in middle
//! Board `i / 81`; Local board `b` lies in middle Board `b / 9`. Legal moves are found by trying
//! every Cell rather than by walking up from the Sent target.

use std::ops::Range;

use kokonoe_engine::{
    Board, BoardStatus, Cell, Continuation, Depth, DrawnOwner, Fallback, Objective, Opening,
    Outcome, Player, Ruleset, Sent, State, Status,
};

use super::{has_line, opponent};

/// Where a Free move may go: inside the target's middle Board, or anywhere.
#[derive(Debug, PartialEq, Eq)]
pub enum Within {
    Middle(usize),
    Global,
}

/// The Sent constraint as the model sees it: Local boards by index, 0 to 80.
#[derive(Debug, PartialEq, Eq)]
pub enum DeepSent {
    FirstMove,
    To(usize),
    Free { target: usize, within: Within },
}

pub struct DeepModel {
    rules: Ruleset,
    cells: [Option<Player>; 729],
    locals: [BoardStatus; 81],
    middles: [BoardStatus; 9],
    last: Option<usize>,
    ply: usize,
    outcome: Option<Outcome>,
}

impl DeepModel {
    pub fn new(rules: Ruleset) -> DeepModel {
        assert_eq!(rules.depth, Depth::Three, "this model plays depth 3 only");
        assert_eq!(rules.opening, Opening::Free, "the model deals no openings");
        DeepModel {
            rules,
            cells: [None; 729],
            locals: [BoardStatus::Open; 81],
            middles: [BoardStatus::Open; 9],
            last: None,
            ply: 0,
            outcome: None,
        }
    }

    pub fn to_move(&self) -> Player {
        if self.ply.is_multiple_of(2) {
            Player::X
        } else {
            Player::O
        }
    }

    /// Whether `cell` can take a mark: it's empty and, with Continuation off, neither Board above
    /// it is decided.
    fn open(&self, cell: usize) -> bool {
        let undecided = self.locals[cell / 9] == BoardStatus::Open
            && self.middles[cell / 81] == BoardStatus::Open;
        self.cells[cell].is_none() && (undecided || self.rules.continuation == Continuation::On)
    }

    fn open_in(&self, cells: Range<usize>) -> Vec<usize> {
        cells.filter(|&cell| self.open(cell)).collect()
    }

    /// The Sent constraint and the legal Cells in address order, or `None` once the game is over.
    /// A move Sends to the Local board its last two Spaces name; when that Board can't take a
    /// move, the next player may play inside its middle Board, and failing that anywhere.
    pub fn turn(&self) -> Option<(DeepSent, Vec<usize>)> {
        if self.outcome.is_some() {
            return None;
        }
        let Some(last) = self.last else {
            return Some((DeepSent::FirstMove, self.open_in(0..729)));
        };
        let target = last % 81;
        let in_target = self.open_in(target * 9..target * 9 + 9);
        if !in_target.is_empty() {
            return Some((DeepSent::To(target), in_target));
        }
        let middle = target / 9;
        let in_middle = self.open_in(middle * 81..middle * 81 + 81);
        if !in_middle.is_empty() {
            let within = Within::Middle(middle);
            return Some((DeepSent::Free { target, within }, in_middle));
        }
        let within = Within::Global;
        Some((DeepSent::Free { target, within }, self.open_in(0..729)))
    }

    /// Whether a Board's status counts toward `player`'s lines and counts: one it won, or a drawn
    /// one when drawn Boards count for both.
    fn counts_for(&self, status: BoardStatus, player: Player) -> bool {
        status == BoardStatus::Won(player)
            || (self.rules.drawn_owner == DrawnOwner::Both && status == BoardStatus::Drawn)
    }

    pub fn won_count(&self, player: Player) -> usize {
        self.middles
            .iter()
            .filter(|&&status| self.counts_for(status, player))
            .count()
    }

    fn meets_objective(&self, player: Player) -> bool {
        match self.rules.objective {
            Objective::Line => has_line(|space| self.counts_for(self.middles[space], player)),
            Objective::FirstTo(goal) => self.won_count(player) >= usize::from(goal.get()),
        }
    }

    /// Plays a Cell the model's own `turn` lists.
    pub fn play(&mut self, cell: usize) {
        let mover = self.to_move();
        self.cells[cell] = Some(mover);
        self.last = Some(cell);
        self.ply += 1;

        // The Local board: a line of the mover's marks, or full. A decided one never changes.
        let local = cell / 9;
        if self.locals[local] != BoardStatus::Open {
            return;
        }
        let empties = (local * 9..local * 9 + 9).any(|cell| self.cells[cell].is_none());
        self.locals[local] = if has_line(|space| self.cells[local * 9 + space] == Some(mover)) {
            BoardStatus::Won(mover)
        } else if !empties {
            BoardStatus::Drawn
        } else {
            return;
        };

        // Its middle Board: a line of won children, the mover's before the opponent's, or every
        // child decided.
        let middle = local / 9;
        if self.middles[middle] != BoardStatus::Open {
            return;
        }
        let children = &self.locals[middle * 9..middle * 9 + 9];
        let line = |player| has_line(|space| self.counts_for(children[space], player));
        self.middles[middle] = if line(mover) {
            BoardStatus::Won(mover)
        } else if line(opponent(mover)) {
            BoardStatus::Won(opponent(mover))
        } else if children.iter().all(|&status| status != BoardStatus::Open) {
            BoardStatus::Drawn
        } else {
            return;
        };

        // A child of the Global board was just decided: the objective, mover first, then the end.
        self.outcome = if self.meets_objective(mover) {
            Some(Outcome::Objective(mover))
        } else if self.meets_objective(opponent(mover)) {
            Some(Outcome::Objective(opponent(mover)))
        } else if self
            .middles
            .iter()
            .all(|&status| status != BoardStatus::Open)
        {
            Some(self.fallback())
        } else {
            None
        };
    }

    fn fallback(&self) -> Outcome {
        let (x, o) = (self.won_count(Player::X), self.won_count(Player::O));
        match self.rules.fallback {
            Fallback::Draw => Outcome::Draw,
            Fallback::MostWon if x > o => Outcome::MostWon(Player::X),
            Fallback::MostWon if o > x => Outcome::MostWon(Player::O),
            Fallback::MostWon => Outcome::Draw,
        }
    }

    /// The first way `state` disagrees with the model, if any: the turn, every mark, all 90
    /// Boards' statuses, the counts and the outcome.
    pub fn disagreement(&self, state: &State) -> Option<String> {
        let differ = |what: &str, engine: &dyn std::fmt::Debug, model: &dyn std::fmt::Debug| {
            Some(format!("{what}: engine {engine:?}, model {model:?}"))
        };
        let path = |board: Board| -> Vec<usize> {
            board
                .spaces()
                .iter()
                .map(|space| usize::from(space.index()))
                .collect()
        };
        // Anything but a Local board reads as an index no Local board has, so it can't agree.
        let local_index = |board: Board| match path(board)[..] {
            [middle, local] => middle * 9 + local,
            _ => usize::MAX,
        };

        let engine_turn = match state.status() {
            Status::ToMove {
                player,
                sent,
                moves,
            } => {
                let sent = match sent {
                    Sent::FirstMove => DeepSent::FirstMove,
                    Sent::To(target) => DeepSent::To(local_index(target)),
                    Sent::Free { target, within } => DeepSent::Free {
                        target: local_index(target),
                        within: match path(within)[..] {
                            [] => Within::Global,
                            [middle] => Within::Middle(middle),
                            _ => return differ("Free within", &within, &"a middle Board"),
                        },
                    },
                };
                let cells: Vec<usize> =
                    moves.iter().map(|cell| usize::from(cell.index())).collect();
                Some((player, sent, cells))
            }
            Status::Over(_) => None,
        };
        let model_turn = self
            .turn()
            .map(|(sent, cells)| (self.to_move(), sent, cells));
        if engine_turn != model_turn {
            return differ("turn", &engine_turn, &model_turn);
        }

        let engine_outcome = match state.status() {
            Status::Over(outcome) => Some(outcome),
            Status::ToMove { .. } => None,
        };
        if engine_outcome != self.outcome {
            return differ("outcome", &engine_outcome, &self.outcome);
        }
        let global = Board::global(Depth::Three);
        let global_status = match self.outcome {
            None => BoardStatus::Open,
            Some(outcome) => outcome
                .winner()
                .map_or(BoardStatus::Drawn, BoardStatus::Won),
        };
        if state.board_status(global) != global_status {
            return differ("Global board", &state.board_status(global), &global_status);
        }

        for (middle, &status) in self.middles.iter().enumerate() {
            let board = Board::from_path(Depth::Three, &[middle as u8]).unwrap();
            if state.board_status(board) != status {
                let what = format!("middle Board {middle}");
                return differ(&what, &state.board_status(board), &status);
            }
        }
        for (local, &status) in self.locals.iter().enumerate() {
            let board =
                Board::from_path(Depth::Three, &[(local / 9) as u8, (local % 9) as u8]).unwrap();
            if state.board_status(board) != status {
                let what = format!("Local board {}·{}", local / 9, local % 9);
                return differ(&what, &state.board_status(board), &status);
            }
        }
        for (index, &mark) in self.cells.iter().enumerate() {
            let cell = Cell::from_index(Depth::Three, index as u16).unwrap();
            if state.mark(cell) != mark {
                return differ(&format!("Cell {cell}"), &state.mark(cell), &mark);
            }
        }
        for player in [Player::X, Player::O] {
            let count = usize::from(state.won_count(player));
            if count != self.won_count(player) {
                let what = format!("won_count({player:?})");
                return differ(&what, &count, &self.won_count(player));
            }
        }
        None
    }
}
