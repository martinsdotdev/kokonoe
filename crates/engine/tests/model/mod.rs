//! A reference model of the depth-2 rules (spec §2.1), written as plainly as possible and
//! sharing no code with the engine: arrays and loops, no masks, no relaxation walk. Tests play it
//! beside the engine and ask for the first disagreement after every ply. `deep` is its depth-3
//! counterpart.

// Each test crate compiles its own copy and uses only part of it.
#![allow(dead_code)]

pub mod deep;

use kokonoe_engine::{
    Board, BoardStatus, Cell, Child, Continuation, Depth, DrawnOwner, Fallback, Goal, Objective,
    Opening, Outcome, Player, Ruleset, Sent, Space, State, Status,
};

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

fn has_line(owned: impl Fn(usize) -> bool) -> bool {
    LINES
        .iter()
        .any(|line| line.iter().all(|&space| owned(space)))
}

fn opponent(player: Player) -> Player {
    match player {
        Player::X => Player::O,
        Player::O => Player::X,
    }
}

/// Every depth-2 Ruleset with a Free opening: 2 × 2 × 10 × 2 = 80 of them.
pub fn depth_two_rulesets() -> Vec<Ruleset> {
    rulesets(Depth::Two)
}

/// Every depth-3 Ruleset with a Free opening, 80 of them too.
pub fn depth_three_rulesets() -> Vec<Ruleset> {
    rulesets(Depth::Three)
}

fn rulesets(depth: Depth) -> Vec<Ruleset> {
    let mut all = Vec::new();
    for continuation in [Continuation::Off, Continuation::On] {
        for drawn_owner in [DrawnOwner::Neither, DrawnOwner::Both] {
            let goals = (1..=9).map(|n| Objective::FirstTo(Goal::try_from(n).unwrap()));
            for objective in [Objective::Line].into_iter().chain(goals) {
                for fallback in [Fallback::Draw, Fallback::MostWon] {
                    all.push(Ruleset {
                        depth,
                        continuation,
                        drawn_owner,
                        objective,
                        fallback,
                        opening: Opening::Free,
                    });
                }
            }
        }
    }
    all
}

/// The Sent constraint as the model sees it: Local boards by their Space.
#[derive(Debug, PartialEq, Eq)]
pub enum ModelSent {
    FirstMove,
    To(usize),
    Free(usize),
}

pub struct Model {
    rules: Ruleset,
    cells: [Option<Player>; 81],
    locals: [BoardStatus; 9],
    last: Option<usize>,
    ply: usize,
    outcome: Option<Outcome>,
}

impl Model {
    pub fn new(rules: Ruleset) -> Model {
        assert_eq!(rules.depth, Depth::Two, "the model plays depth 2 only");
        assert_eq!(rules.opening, Opening::Free, "the model deals no openings");
        Model {
            rules,
            cells: [None; 81],
            locals: [BoardStatus::Open; 9],
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

    fn empties(&self, board: usize) -> impl Iterator<Item = usize> + '_ {
        (board * 9..board * 9 + 9).filter(|&cell| self.cells[cell].is_none())
    }

    /// A Local board takes a move when it has an empty Cell and is Open, or Continuation is on.
    fn takes_moves(&self, board: usize) -> bool {
        let playable =
            self.locals[board] == BoardStatus::Open || self.rules.continuation == Continuation::On;
        playable && self.empties(board).next().is_some()
    }

    /// The Sent constraint and the legal Cells in address order, or `None` once the game is over.
    pub fn turn(&self) -> Option<(ModelSent, Vec<usize>)> {
        if self.outcome.is_some() {
            return None;
        }
        let anywhere = || -> Vec<usize> {
            (0..9)
                .filter(|&board| self.takes_moves(board))
                .flat_map(|board| self.empties(board))
                .collect()
        };
        Some(match self.last {
            None => (ModelSent::FirstMove, anywhere()),
            Some(last) => {
                let target = last % 9;
                if self.takes_moves(target) {
                    (ModelSent::To(target), self.empties(target).collect())
                } else {
                    (ModelSent::Free(target), anywhere())
                }
            }
        })
    }

    /// The children of the Global board that count for `player`: the ones it won, and the drawn
    /// ones when drawn Boards count for both.
    fn counts(&self, player: Player) -> [bool; 9] {
        let both = self.rules.drawn_owner == DrawnOwner::Both;
        self.locals.map(|status| {
            status == BoardStatus::Won(player) || (both && status == BoardStatus::Drawn)
        })
    }

    pub fn won_count(&self, player: Player) -> usize {
        self.counts(player).iter().filter(|&&counts| counts).count()
    }

    fn meets_objective(&self, player: Player) -> bool {
        let counts = self.counts(player);
        match self.rules.objective {
            Objective::Line => has_line(|space| counts[space]),
            Objective::FirstTo(goal) => self.won_count(player) >= usize::from(goal.get()),
        }
    }

    /// Plays a Cell the model's own `turn` lists.
    pub fn play(&mut self, cell: usize) {
        let mover = self.to_move();
        let board = cell / 9;
        self.cells[cell] = Some(mover);
        self.last = Some(cell);
        self.ply += 1;
        // A Decided Board never changes: under Continuation the mark lands, and that's all.
        if self.locals[board] != BoardStatus::Open {
            return;
        }
        let marks = |player| {
            let cells = &self.cells;
            move |space: usize| cells[board * 9 + space] == Some(player)
        };
        self.locals[board] = if has_line(marks(mover)) {
            BoardStatus::Won(mover)
        } else if self.empties(board).next().is_none() {
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
            .locals
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

    /// The first way `state` disagrees with the model, if any: the turn, every mark, every
    /// Board's status, the counts and the outcome.
    pub fn disagreement(&self, state: &State) -> Option<String> {
        let space_of = |board: Board| usize::from(board.spaces()[0].index());
        let global = Board::global(Depth::Two);
        let differ = |what: &str, engine: &dyn std::fmt::Debug, model: &dyn std::fmt::Debug| {
            Some(format!("{what}: engine {engine:?}, model {model:?}"))
        };

        let engine_turn = match state.status() {
            Status::ToMove {
                player,
                sent,
                moves,
            } => {
                let sent = match sent {
                    Sent::FirstMove => ModelSent::FirstMove,
                    Sent::To(target) => ModelSent::To(space_of(target)),
                    Sent::Free { target, within } => {
                        if within != global {
                            return differ("Free within", &within, &global);
                        }
                        ModelSent::Free(space_of(target))
                    }
                };
                // At depth 2 a Cell's index is its Local board's Space times 9 plus its own.
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
        let global_status = match self.outcome {
            None => BoardStatus::Open,
            Some(outcome) => outcome
                .winner()
                .map_or(BoardStatus::Drawn, BoardStatus::Won),
        };
        if state.board_status(global) != global_status {
            return differ("Global board", &state.board_status(global), &global_status);
        }

        for (board, &status) in self.locals.iter().enumerate() {
            let local = local(board);
            if state.board_status(local) != status {
                return differ(
                    &format!("Local board {board}"),
                    &state.board_status(local),
                    &status,
                );
            }
            for space in 0..9 {
                let cell = cell(board, space);
                let mark = self.cells[board * 9 + space];
                if state.mark(cell) != mark {
                    return differ(&format!("Cell {board}·{space}"), &state.mark(cell), &mark);
                }
            }
        }
        for player in [Player::X, Player::O] {
            let count = usize::from(state.won_count(player));
            if count != self.won_count(player) {
                return differ(
                    &format!("won_count({player:?})"),
                    &count,
                    &self.won_count(player),
                );
            }
        }
        None
    }
}

/// Local board `board` of a depth-2 game.
pub fn local(board: usize) -> Board {
    let space = Space::ALL[board];
    match Board::global(Depth::Two).child(space) {
        Child::Board(local) => local,
        Child::Cell(_) => unreachable!(),
    }
}

/// The Cell at `space` on Local board `board`.
pub fn cell(board: usize, space: usize) -> Cell {
    match local(board).child(Space::ALL[space]) {
        Child::Cell(cell) => cell,
        Child::Board(_) => unreachable!(),
    }
}
