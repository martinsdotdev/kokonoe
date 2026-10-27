//! Play. A [`Record`] is a game as data. [`Game`] replays a Record and owns the current
//! [`State`], the `Copy` value the AI searches and the UI renders.

use alloc::vec::Vec;
use core::ops::Range;

use crate::address::{Board, Cell, Child, Space};
use crate::error::{IllegalMove, OpenError, PlayError, ReplayError};
use crate::repr::{BoardRepr, Masks, Repr, has_line};
use crate::rng::Rng;
use crate::ruleset::{Continuation, Depth, Fallback, Objective, Opening, Ruleset};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Player {
    X,
    O,
}

impl Player {
    pub const fn opponent(self) -> Player {
        match self {
            Player::X => Player::O,
            Player::O => Player::X,
        }
    }
}

/// How many marks are on the board, random opening included, and so the index of the next one.
///
/// A newtype so a ply is never confused with an index into a Record's moves, which exclude the
/// opening.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Ply(u16);

impl Ply {
    pub const START: Ply = Ply(0);

    pub const fn new(n: u16) -> Ply {
        Ply(n)
    }

    pub const fn get(self) -> u16 {
        self.0
    }

    /// Turn order, written once: X moves first and turns alternate.
    pub const fn player(self) -> Player {
        if self.0.is_multiple_of(2) {
            Player::X
        } else {
            Player::O
        }
    }
}

/// Drives a Random opening. Part of the game record, beside the Ruleset and the moves. The
/// caller supplies it, since the engine has no entropy source. A server draws it, so neither
/// client picks its own opening. Under `Opening::Free` it is carried and unused.
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(try_from = "alloc::string::String", into = "alloc::string::String")
)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Seed(pub u64);

/// A game as data: what a Room persists, an export carries, a Worker receives, and a rating
/// check replays. Its JSON is spec §4.3's, with the moves as Cell indices.
///
/// Claimed, not checked. The moves may be illegal until [`Game::open`] says otherwise. The random
/// opening is not in `moves`: it is derived from `seed`, so it has one source of truth and no
/// client can forge it.
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(
        try_from = "crate::encoding::RawRecord",
        into = "crate::encoding::RawRecord"
    )
)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Record {
    /// The `RULES_VERSION` it was played under. [`Game::open`] plays only its own.
    pub rules_version: u32,
    pub ruleset: Ruleset,
    pub seed: Seed,
    /// The moves players made after the opening, in order.
    pub moves: Vec<Cell>,
}

/// A live game: a Record, the [`State`] it leads to, and the history between them. What a Room
/// and the UI hold.
///
/// Invariants: `state` is what `Game::new(ruleset, seed)` followed by `moves` produces, and
/// `opening` holds the random plies `new` drew. Only `new`, `open`, `play`, and `undo` write.
#[derive(Clone)]
pub struct Game {
    seed: Seed,
    opening: Vec<Cell>,
    moves: Vec<Cell>,
    state: State,
}

impl Game {
    /// Starts a game. Under `Opening::Random(k)` it also plays the k random plies, so the first
    /// move a player makes is ply k.
    pub fn new(ruleset: Ruleset, seed: Seed) -> Game {
        let mut state = State::start(ruleset);
        let mut rng = Rng::new(seed);
        let plies = match ruleset.opening {
            Opening::Free => 0,
            Opening::Random(k) => k.get(),
        };
        let mut opening = Vec::new();
        for _ in 0..plies {
            // FirstTo(1) can end the game inside the opening: X can win a Local board on its third
            // mark, the fifth ply.
            let Status::ToMove { moves, .. } = state.status else {
                break;
            };
            // Uniform over this turn's legal Cells, in address order. Both the order and the
            // generator are part of RULES_VERSION.
            let cell = moves.nth(rng.below(moves.len()));
            if state.play(cell).is_err() {
                break; // unreachable: `cell` came from `moves`
            }
            opening.push(cell);
        }
        Game {
            seed,
            opening,
            moves: Vec::new(),
            state,
        }
    }

    /// Rebuilds a game from an untrusted Record: `new`, then [`State::play`] for each move. A
    /// Record from another `RULES_VERSION` is refused, since it could replay differently here,
    /// and the first illegal move stops it with its index in `record.moves`. The one path for
    /// reconnects, deploy restarts, imports, Workers, and rating checks.
    pub fn open(record: &Record) -> Result<Game, OpenError> {
        if record.rules_version != crate::RULES_VERSION {
            return Err(OpenError::RulesVersion(record.rules_version));
        }
        let mut game = Game::new(record.ruleset, record.seed);
        for (index, &cell) in record.moves.iter().enumerate() {
            game.state
                .play(cell)
                .map_err(|error| OpenError::Illegal(ReplayError { index, error }))?;
            game.moves.push(cell);
        }
        Ok(game)
    }

    /// The data to persist, export, or rate. Allocates; call it when saving, not per move.
    pub fn record(&self) -> Record {
        Record {
            rules_version: crate::RULES_VERSION,
            ruleset: self.state.ruleset,
            seed: self.seed,
            moves: self.moves.clone(),
        }
    }

    /// Plays `cell` at ply `at` for the side to move, and appends it to the record.
    ///
    /// The ply stamp makes a resend idempotent: a client that resends after a reconnect, or a
    /// double tap in the UI, gets `Already` and changes nothing.
    ///
    /// ```text
    /// at == state.ply()                       -> state.play(cell)?, push, Ok(Now(status))
    /// at <  state.ply() && history[at] == cell -> Ok(Already)     // history = opening ++ moves
    /// otherwise                               -> Err(OutOfOrder { want: state.ply(), got: at })
    /// ```
    ///
    /// All or nothing: on `Err` nothing changed.
    pub fn play(&mut self, at: Ply, cell: Cell) -> Result<Played, PlayError> {
        let now = self.state.ply;
        if at == now {
            let status = self.state.play(cell)?;
            self.moves.push(cell);
            return Ok(Played::Now(status));
        }
        if at < now && self.history().nth(usize::from(at.get())) == Some(cell) {
            return Ok(Played::Already);
        }
        Err(PlayError::OutOfOrder { want: now, got: at })
    }

    /// Takes back the last move a player made and returns it, rebuilding the State by replay.
    /// `None` when only the random opening remains: those plies are setup, not anyone's move.
    pub fn undo(&mut self) -> Option<Cell> {
        let cell = self.moves.pop()?;
        self.state = replay(self.state.ruleset, self.history());
        Some(cell)
    }

    pub fn state(&self) -> &State {
        &self.state
    }

    /// The State after `ply` plies, opening included, where `Ply::START` is the empty board. For
    /// scrubbing through a replay. `None` past the end.
    pub fn state_at(&self, ply: Ply) -> Option<State> {
        if ply > self.state.ply {
            return None;
        }
        let plies = usize::from(ply.get());
        Some(replay(self.state.ruleset, self.history().take(plies)))
    }

    pub fn seed(&self) -> Seed {
        self.seed
    }

    /// The random opening's plies, in order. Empty under `Opening::Free`.
    pub fn opening(&self) -> &[Cell] {
        &self.opening
    }

    /// The moves players made after the opening, in order.
    pub fn moves(&self) -> &[Cell] {
        &self.moves
    }

    /// Every ply in order: the opening, then the moves.
    fn history(&self) -> impl Iterator<Item = Cell> + '_ {
        self.opening.iter().chain(&self.moves).copied()
    }
}

/// The State a legal history leads to.
fn replay(ruleset: Ruleset, history: impl Iterator<Item = Cell>) -> State {
    let mut state = State::start(ruleset);
    for cell in history {
        if state.play(cell).is_err() {
            break; // unreachable: every ply of a Game's history was legal when it was played
        }
    }
    state
}

/// What [`Game::play`] did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Played {
    /// The move was applied; this is the new status.
    Now(Status),
    /// This exact move was already at this ply. Nothing changed.
    Already,
}

/// The game at one moment: what the AI searches and the UI renders.
///
/// `Copy` and heap-free by construction, so a copy per playout or per tree node is a memcpy of a
/// few hundred bytes. It has no public constructor: every State comes from a [`Game`], so every
/// State is reachable under its Ruleset. The only way forward is [`State::play`]. No `Debug`,
/// because printing it would print the representation.
///
/// Invariants: `play` is the single writer. `status` is recomputed by every `play` from the marks
/// and the last move, and `ply` counts the marks placed.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct State {
    ruleset: Ruleset,
    ply: Ply,
    status: Status,
    repr: Repr,
}

impl State {
    /// The empty board: X to move, `Sent::FirstMove`, every Cell legal.
    fn start(ruleset: Ruleset) -> State {
        let repr = Repr::empty(ruleset.depth);
        let global = Board::global(ruleset.depth);
        let status = match repr.legal_in(global, ruleset.continuation) {
            Some(moves) => Status::ToMove {
                player: Player::X,
                sent: Sent::FirstMove,
                moves,
            },
            None => Status::Over(Outcome::Draw), // unreachable: an empty board has every Cell
        };
        State {
            ruleset,
            ply: Ply::START,
            status,
            repr,
        }
    }

    pub fn ruleset(&self) -> Ruleset {
        self.ruleset
    }

    /// Whose turn it is, under which constraint, with which moves; or the outcome. A read, not a
    /// computation: `play` already did the work.
    pub fn status(&self) -> Status {
        self.status
    }

    /// Plays `cell` for the side to move and returns the new status. All or nothing: on `Err`
    /// the State is unchanged. The AI's hot path, so validation is one bit test.
    pub fn play(&mut self, cell: Cell) -> Result<Status, IllegalMove> {
        let Status::ToMove { player, moves, .. } = self.status else {
            return Err(IllegalMove::GameOver);
        };
        if !moves.contains(cell) {
            return Err(self.why_illegal(cell));
        }
        // Nothing below can fail, so a rejected move leaves `self` untouched.
        let decided = self.repr.place(cell, player, self.ruleset.drawn_owner);
        self.ply = Ply(self.ply.0.wrapping_add(1)); // at most 729

        // The game can only end when a child of the Global board was newly decided.
        if decided.is_some_and(|board| board.parent().is_some_and(Board::is_global)) {
            if let Some(winner) = self.objective_winner(player) {
                self.status = Status::Over(Outcome::Objective(winner));
                return Ok(self.status);
            }
            if self
                .children()
                .all(|child| self.repr.status(child).is_decided())
            {
                // Nothing can change any more. With continuation off this is also the moment no
                // legal move remains, so a turn with no moves never exists.
                self.status = Status::Over(self.fallback());
                return Ok(self.status);
            }
        }

        // Sent, then relaxation: walk up from the target to the nearest Board with a legal Cell.
        // Some child of the Global board is still open, so the walk always finds one.
        let target = cell.sends_to();
        let mut within = target;
        self.status = loop {
            if let Some(moves) = self.repr.legal_in(within, self.ruleset.continuation) {
                let sent = if within == target {
                    Sent::To(target)
                } else {
                    Sent::Free { target, within }
                };
                break Status::ToMove {
                    player: player.opponent(),
                    sent,
                    moves,
                };
            }
            match within.parent() {
                Some(up) => within = up,
                None => break Status::Over(self.fallback()), // unreachable, kept total
            }
        };
        Ok(self.status)
    }

    /// `None` for an empty Cell, and for a Cell of the other depth.
    pub fn mark(&self, cell: Cell) -> Option<Player> {
        if cell.depth() != self.ruleset.depth {
            return None;
        }
        self.repr.mark(cell)
    }

    /// Open, Won, or Drawn, for a Board at any level. Decided Boards never change. A Board of the
    /// other depth is Open.
    ///
    /// The Global board's status is the game's outcome under every objective: Open while the
    /// game goes on, Won by the winner, Drawn on a draw. A line among its children decides it
    /// only under `Objective::Line`.
    pub fn board_status(&self, board: Board) -> BoardStatus {
        if board.depth() != self.ruleset.depth {
            return BoardStatus::Open;
        }
        if !board.is_global() {
            return self.repr.status(board);
        }
        match self.status {
            Status::ToMove { .. } => BoardStatus::Open,
            Status::Over(outcome) => match outcome.winner() {
                Some(player) => BoardStatus::Won(player),
                None => BoardStatus::Drawn,
            },
        }
    }

    /// How many of the Global board's children count as won for `player`: the ones it won, plus
    /// the drawn ones under `DrawnOwner::Both`. This is what `FirstTo` and `MostWon` compare.
    pub fn won_count(&self, player: Player) -> u8 {
        let (mine, _) = self.global_masks().lines(player, self.ruleset.drawn_owner);
        mine.count_ones() as u8
    }

    pub fn ply(&self) -> Ply {
        self.ply
    }

    /// The children of the Global board.
    fn children(&self) -> impl Iterator<Item = Board> {
        let global = Board::global(self.ruleset.depth);
        Space::ALL
            .into_iter()
            .filter_map(move |space| match global.child(space) {
                Child::Board(board) => Some(board),
                Child::Cell(_) => None,
            })
    }

    /// The Global board's children as masks, the way every other Board's are kept.
    fn global_masks(&self) -> Masks {
        let mut masks = Masks {
            x: 0,
            o: 0,
            drawn: 0,
        };
        for child in self.children() {
            let bit = child.space().bit();
            match self.repr.status(child) {
                BoardStatus::Won(Player::X) => masks.x |= bit,
                BoardStatus::Won(Player::O) => masks.o |= bit,
                BoardStatus::Drawn => masks.drawn |= bit,
                BoardStatus::Open => {}
            }
        }
        masks
    }

    /// Why `cell` is not in this turn's legal moves. Checked in this order: `GameOver`,
    /// `WrongDepth`, `Occupied`, `BeneathDecided` (continuation off), `OutsideActiveBoards`.
    /// Runs only on the failure path.
    fn why_illegal(&self, cell: Cell) -> IllegalMove {
        let Status::ToMove { sent, .. } = self.status else {
            return IllegalMove::GameOver;
        };
        if cell.depth() != self.ruleset.depth {
            return IllegalMove::WrongDepth;
        }
        if self.repr.mark(cell).is_some() {
            return IllegalMove::Occupied;
        }
        if self.ruleset.continuation == Continuation::Off {
            let mut outermost = None;
            let mut board = cell.local_board();
            while let Some(parent) = board.parent() {
                if self.repr.status(board).is_decided() {
                    outermost = Some(board);
                }
                board = parent;
            }
            if let Some(board) = outermost {
                return IllegalMove::BeneathDecided { board };
            }
        }
        let within = match sent {
            Sent::FirstMove => Board::global(self.ruleset.depth),
            Sent::To(target) => target,
            Sent::Free { within, .. } => within,
        };
        IllegalMove::OutsideActiveBoards { within }
    }

    /// The objective after `mover`'s move, reading the Global board's children with a drawn
    /// child counting for both under `DrawnOwner::Both`. `mover` is checked first, so a double
    /// win goes to the mover; the opponent can also meet it on this move alone, through a child
    /// that became drawn under `Both`.
    fn objective_winner(&self, mover: Player) -> Option<Player> {
        let (mine, theirs) = self.global_masks().lines(mover, self.ruleset.drawn_owner);
        let met = |mask: u16| match self.ruleset.objective {
            Objective::Line => has_line(mask),
            Objective::FirstTo(goal) => mask.count_ones() >= u32::from(goal.get()),
        };
        if met(mine) {
            Some(mover)
        } else if met(theirs) {
            Some(mover.opponent())
        } else {
            None
        }
    }

    /// Draw, or MostWon on `won_count` with equal counts drawing.
    fn fallback(&self) -> Outcome {
        match self.ruleset.fallback {
            Fallback::Draw => Outcome::Draw,
            Fallback::MostWon => {
                let (x, o) = (self.won_count(Player::X), self.won_count(Player::O));
                if x > o {
                    Outcome::MostWon(Player::X)
                } else if o > x {
                    Outcome::MostWon(Player::O)
                } else {
                    Outcome::Draw
                }
            }
        }
    }
}

/// The game's progress. One value, so "whose turn" and "is it over" can't disagree.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    /// The game goes on. `moves` holds exactly the legal Cells, all inside the Board that `sent`
    /// allows, and is never empty.
    ToMove {
        player: Player,
        sent: Sent,
        moves: LegalMoves,
    },
    Over(Outcome),
}

/// The constraint the last move places on the side to move.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Sent {
    /// The game's first move. Nobody has been Sent anywhere, so it may go anywhere.
    FirstMove,
    /// Held to the Sent target, a Local board that can take a move.
    To(Board),
    /// A Free move. The Sent target can take no move, so the player may play anywhere legal
    /// inside `within`: the nearest Board above `target` that still has a legal Cell. The Global
    /// board means anywhere. At depth 2 `within` is always the Global board.
    Free { target: Board, within: Board },
}

/// The legal Cells of one turn. Never empty, and iterated in ascending address order.
///
/// The order is a seam invariant every representation must honour (`BoardRepr::legal_in`),
/// because the random opening picks by position in it. Opaque, so the bitset can change; today
/// it is one bit per Cell.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LegalMoves {
    depth: Depth,
    bits: [u64; 12],
}

impl LegalMoves {
    /// `None` when no bit is set. The only constructor, so emptiness can't get past it.
    pub(crate) fn from_bits(depth: Depth, bits: [u64; 12]) -> Option<LegalMoves> {
        bits.iter()
            .any(|&word| word != 0)
            .then_some(LegalMoves { depth, bits })
    }

    /// At least 1.
    #[expect(clippy::len_without_is_empty, reason = "a LegalMoves is never empty")]
    pub fn len(&self) -> usize {
        self.bits
            .iter()
            .map(|word| word.count_ones() as usize)
            .sum()
    }

    /// These moves without `cell`. `None` when `cell` was the only one left.
    pub(crate) fn without(self, cell: Cell) -> Option<LegalMoves> {
        let mut bits = self.bits;
        let index = cell.index();
        if cell.depth() == self.depth
            && let Some(word) = bits.get_mut(usize::from(index / 64))
        {
            *word &= !(1 << (index % 64));
        }
        LegalMoves::from_bits(self.depth, bits)
    }

    pub fn contains(&self, cell: Cell) -> bool {
        let index = cell.index();
        cell.depth() == self.depth
            && self
                .bits
                .get(usize::from(index / 64))
                .is_some_and(|word| word >> (index % 64) & 1 == 1)
    }

    /// The `(i mod len)`-th legal Cell in address order. Total, because a LegalMoves is never
    /// empty: a uniform `i` in `0..len()` gives a uniform move.
    pub fn nth(&self, i: usize) -> Cell {
        let mut k = i.checked_rem(self.len()).unwrap_or(0);
        for (w, &word) in (0u16..).zip(&self.bits) {
            let count = word.count_ones() as usize;
            if k < count {
                let mut word = word;
                for _ in 0..k {
                    word &= word.wrapping_sub(1); // clear the lowest set bit
                }
                // At most 11 · 64 + 63.
                let index = w
                    .wrapping_mul(64)
                    .wrapping_add(word.trailing_zeros() as u16);
                return Cell::new(self.depth, index);
            }
            k = k.wrapping_sub(count);
        }
        Cell::new(self.depth, 0) // unreachable: k < len
    }

    pub fn iter(&self) -> LegalIter {
        LegalIter {
            moves: *self,
            next_bit: 0,
        }
    }

    /// Whether any legal Cell lies inside `board`, at any level. For a Local board this is "is it
    /// one of the Active boards". Derived from the moves, so it can't drift from them, and a
    /// Board's Cells are a contiguous run of bits, so this is a masked test per word.
    pub fn any_in(&self, board: Board) -> bool {
        if board.depth() != self.depth {
            return false;
        }
        let cells = board.cells();
        (0u16..)
            .zip(&self.bits)
            .any(|(w, &word)| word & word_mask(w, &cells) != 0)
    }
}

/// The bits of word `w` whose indices lie in `cells`.
fn word_mask(w: u16, cells: &Range<u16>) -> u64 {
    let first = w.wrapping_mul(64);
    let start = cells.start.max(first);
    let end = cells.end.min(first.wrapping_add(64));
    if start >= end {
        return 0;
    }
    // `end - start` is 1 to 64, and `start - first` is 0 to 63.
    let ones = u64::MAX >> 64u16.wrapping_sub(end.wrapping_sub(start));
    ones << start.wrapping_sub(first)
}

/// Iterates a [`LegalMoves`] in ascending address order.
#[derive(Clone, Debug)]
pub struct LegalIter {
    moves: LegalMoves,
    next_bit: u16,
}

impl Iterator for LegalIter {
    type Item = Cell;

    fn next(&mut self) -> Option<Cell> {
        let mut index = self.next_bit;
        while let Some(&word) = self.moves.bits.get(usize::from(index / 64)) {
            let rest = word >> (index % 64);
            if rest != 0 {
                // Below 12 · 64.
                let found = index.wrapping_add(rest.trailing_zeros() as u16);
                self.next_bit = found.wrapping_add(1);
                return Some(Cell::new(self.moves.depth, found));
            }
            index = (index / 64).wrapping_add(1).wrapping_mul(64);
        }
        self.next_bit = index;
        None
    }
}

/// The status of one Board. Decided means Won or Drawn, and a Decided Board never changes again:
/// the first line fixes the owner forever.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BoardStatus {
    Open,
    Won(Player),
    Drawn,
}

impl BoardStatus {
    pub const fn is_decided(self) -> bool {
        !matches!(self, BoardStatus::Open)
    }
}

/// How a game ended. A game ends when the objective is met, or when every child of the Global
/// board is Decided and nothing can change any more.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Outcome {
    /// This player met the objective. When one move met it for both players, the mover.
    Objective(Player),
    /// Every child of the Global board was Decided, and under `Fallback::MostWon` this player
    /// had more won children.
    MostWon(Player),
    /// Every child of the Global board was Decided, and the fallback is Draw or the counts were
    /// equal.
    Draw,
}

impl Outcome {
    pub const fn winner(self) -> Option<Player> {
        match self {
            Outcome::Objective(player) | Outcome::MostWon(player) => Some(player),
            Outcome::Draw => None,
        }
    }
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects
)]
mod tests {
    use super::*;

    fn moves(indices: &[u16]) -> LegalMoves {
        let mut bits = [0; 12];
        for &i in indices {
            bits[usize::from(i / 64)] |= 1 << (i % 64);
        }
        LegalMoves::from_bits(Depth::Three, bits).unwrap()
    }

    #[test]
    fn no_bits_make_no_moves() {
        assert!(LegalMoves::from_bits(Depth::Two, [0; 12]).is_none());
    }

    #[test]
    fn nth_and_iter_walk_address_order_across_words() {
        let legal = moves(&[3, 63, 64, 400, 728]);
        let cells: Vec<u16> = legal.iter().map(Cell::index).collect();
        assert_eq!(cells, [3, 63, 64, 400, 728]);
        assert_eq!(legal.len(), 5);
        for (i, &index) in cells.iter().enumerate() {
            assert_eq!(legal.nth(i).index(), index);
            assert_eq!(legal.nth(i + 5).index(), index, "nth wraps around");
        }
    }

    #[test]
    fn any_in_sees_only_the_boards_cells() {
        // Cell 400 is 4·8·4 at depth 3: Local board 4·8 (index 44), inside middle Board 4.
        let legal = moves(&[400]);
        let global = Board::global(Depth::Three);
        let Child::Board(middle) = global.child(Space::ALL[4]) else {
            unreachable!()
        };
        let Child::Board(local) = middle.child(Space::ALL[8]) else {
            unreachable!()
        };
        assert!(legal.any_in(global));
        assert!(legal.any_in(middle));
        assert!(legal.any_in(local));
        let Child::Board(neighbour) = middle.child(Space::ALL[7]) else {
            unreachable!()
        };
        assert!(!legal.any_in(neighbour));
        assert!(!legal.any_in(Board::global(Depth::Two)));
    }
}
