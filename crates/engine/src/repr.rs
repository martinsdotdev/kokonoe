//! The representation seam: the only code that knows how marks and Board statuses are stored.
//!
//! The rules in `play` (turn order, Sent, relaxation, objective, double win, end of game,
//! fallback, opening) are written once against [`BoardRepr`]. Settlement lives here rather than
//! there because it is fixed rules parameterized only by [`DrawnOwner`], answered by 9-bit masks
//! and a 512-entry win table at every level.

use crate::address::{Board, Cell, Child, Space};
use crate::play::{BoardStatus, LegalMoves, Player};
use crate::ruleset::{Continuation, Depth, DrawnOwner};

pub(crate) trait BoardRepr: Copy + Eq {
    /// Every Cell empty, every Board Open.
    fn empty(depth: Depth) -> Self;

    fn mark(&self, cell: Cell) -> Option<Player>;

    /// The status of any Board below the Global board. The Global board's status is the game's
    /// outcome, which `State` owns, so here it is always Open.
    fn status(&self, board: Board) -> BoardStatus;

    /// Writes `mover`'s mark into the empty `cell`, then settles Boards upward from its Local
    /// board, stopping below the Global board, and returns the highest Board this move newly
    /// decided. A Board settles to:
    /// - `Won(p)` on the first line of p's won children, where a drawn child counts for both
    ///   players under `DrawnOwner::Both`. A line for the opponent alone gives it to the
    ///   opponent; lines for both players at once give it to the mover.
    /// - `Drawn` with no line, when it is full (a Local board) or every child is Decided (above).
    /// - `Open` otherwise, which stops the walk.
    ///
    /// A Decided Board is never re-settled, so with continuation on, marks still land inside it
    /// and change nothing above.
    fn place(&mut self, cell: Cell, mover: Player, drawn: DrawnOwner) -> Option<Board>;

    /// The empty Cells inside `scope` that may take a move, or `None` if there are none. With
    /// continuation off, no Cell beneath a Decided Board qualifies, whether that Board is inside
    /// `scope`, is `scope`, or is above it.
    ///
    /// A seam invariant: whatever the representation, the moves iterate in ascending Cell index.
    /// A Random opening deals by position in this list, so another order would deal different
    /// openings from the same Seeds and needs a new `RULES_VERSION`. `tests/openings.rs` pins it.
    fn legal_in(&self, scope: Board, continuation: Continuation) -> Option<LegalMoves>;
}

/// Measured at 400k depth-2 playouts/s in WASM, so v1 has no specialized representation.
pub(crate) type Repr = GenericRepr;

/// Reference representation for any depth. Every Board holds three 9-bit masks over its
/// children: X (marks, or Boards X won), O, and drawn. Boards are in breadth-first order (see
/// `Board::slot`), and a Board's own status is its bit in its parent's masks.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) struct GenericRepr {
    depth: Depth,
    x: [u16; 91],
    o: [u16; 91],
    drawn: [u16; 91],
}

/// One Board's masks over its nine children.
#[derive(Clone, Copy)]
pub(crate) struct Masks {
    pub(crate) x: u16,
    pub(crate) o: u16,
    pub(crate) drawn: u16,
}

const FULL: u16 = 0x1FF;

impl Masks {
    /// The masks `player` and the opponent would complete a line on, a drawn child counting for
    /// both under `DrawnOwner::Both`.
    pub(crate) const fn lines(self, player: Player, drawn: DrawnOwner) -> (u16, u16) {
        let shared = match drawn {
            DrawnOwner::Neither => 0,
            DrawnOwner::Both => self.drawn,
        };
        match player {
            Player::X => (self.x | shared, self.o | shared),
            Player::O => (self.o | shared, self.x | shared),
        }
    }

    /// The Board these masks belong to, settled as `place` describes.
    fn settle(self, mover: Player, drawn: DrawnOwner) -> BoardStatus {
        let (mine, theirs) = self.lines(mover, drawn);
        if has_line(mine) {
            BoardStatus::Won(mover)
        } else if has_line(theirs) {
            BoardStatus::Won(mover.opponent())
        } else if self.x | self.o | self.drawn == FULL {
            BoardStatus::Drawn
        } else {
            BoardStatus::Open
        }
    }
}

impl GenericRepr {
    pub(crate) fn masks(&self, board: Board) -> Masks {
        let slot = board.slot();
        // Every Board's slot is below 91.
        let at = |masks: &[u16; 91]| masks.get(slot).copied().unwrap_or(0);
        Masks {
            x: at(&self.x),
            o: at(&self.o),
            drawn: at(&self.drawn),
        }
    }

    fn set(&mut self, board: Board, space: Space, status: BoardStatus) {
        let masks = match status {
            BoardStatus::Won(Player::X) => &mut self.x,
            BoardStatus::Won(Player::O) => &mut self.o,
            BoardStatus::Drawn => &mut self.drawn,
            BoardStatus::Open => return,
        };
        if let Some(mask) = masks.get_mut(board.slot()) {
            *mask |= space.bit();
        }
    }

    fn decided_at_or_above(&self, mut board: Board) -> bool {
        loop {
            if self.status(board).is_decided() {
                return true;
            }
            match board.parent() {
                Some(parent) => board = parent,
                None => return false,
            }
        }
    }

    /// Sets the bit of every legal Cell under `board`, which is not beneath a Decided Board when
    /// continuation is off.
    fn collect(&self, board: Board, continuation: Continuation, bits: &mut [u64; 12]) {
        let masks = self.masks(board);
        if board.is_local() {
            // A Local board's nine Cells are consecutive bits from its first, so its empty ones
            // go in with one shift, spilling into the next word at most.
            let empty = u64::from(!(masks.x | masks.o) & FULL);
            let start = board.cells().start;
            let (word, offset) = (usize::from(start / 64), u32::from(start % 64));
            if let Some(bits) = bits.get_mut(word) {
                *bits |= empty << offset;
            }
            if offset > 55
                && let Some(bits) = bits.get_mut(word.wrapping_add(1))
            {
                *bits |= empty >> 64u32.wrapping_sub(offset);
            }
            return;
        }
        // A child's status is its bit in these masks, so they're read once for all nine.
        let decided = masks.x | masks.o | masks.drawn;
        for space in Space::ALL {
            if continuation == Continuation::Off && decided & space.bit() != 0 {
                continue;
            }
            if let Child::Board(child) = board.child(space) {
                self.collect(child, continuation, bits);
            }
        }
    }
}

impl BoardRepr for GenericRepr {
    fn empty(depth: Depth) -> Self {
        GenericRepr {
            depth,
            x: [0; 91],
            o: [0; 91],
            drawn: [0; 91],
        }
    }

    fn mark(&self, cell: Cell) -> Option<Player> {
        let masks = self.masks(cell.local_board());
        let bit = cell.space().bit();
        if masks.x & bit != 0 {
            Some(Player::X)
        } else if masks.o & bit != 0 {
            Some(Player::O)
        } else {
            None
        }
    }

    fn status(&self, board: Board) -> BoardStatus {
        let Some(parent) = board.parent() else {
            return BoardStatus::Open;
        };
        let masks = self.masks(parent);
        let bit = board.space().bit();
        if masks.x & bit != 0 {
            BoardStatus::Won(Player::X)
        } else if masks.o & bit != 0 {
            BoardStatus::Won(Player::O)
        } else if masks.drawn & bit != 0 {
            BoardStatus::Drawn
        } else {
            BoardStatus::Open
        }
    }

    fn place(&mut self, cell: Cell, mover: Player, drawn: DrawnOwner) -> Option<Board> {
        let mut board = cell.local_board();
        // A mark is stored like a child the mover won.
        self.set(board, cell.space(), BoardStatus::Won(mover));
        let mut highest = None;
        while let Some(parent) = board.parent() {
            if self.status(board).is_decided() {
                break;
            }
            let settled = self.masks(board).settle(mover, drawn);
            if settled == BoardStatus::Open {
                break;
            }
            self.set(parent, board.space(), settled);
            highest = Some(board);
            board = parent;
        }
        highest
    }

    fn legal_in(&self, scope: Board, continuation: Continuation) -> Option<LegalMoves> {
        if continuation == Continuation::Off && self.decided_at_or_above(scope) {
            return None;
        }
        let mut bits = [0; 12];
        self.collect(scope, continuation, &mut bits);
        LegalMoves::from_bits(self.depth, bits)
    }
}

/// The eight lines of a Board as 9-bit masks over its Spaces.
const LINES: [u16; 8] = [
    0b000_000_111,
    0b000_111_000,
    0b111_000_000,
    0b001_001_001,
    0b010_010_010,
    0b100_100_100,
    0b100_010_001,
    0b001_010_100,
];

/// Whether each 9-bit mask holds a line.
static WIN: [bool; 512] = build_win();

#[allow(
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    reason = "runs at compile time, where a panic fails the build"
)]
const fn build_win() -> [bool; 512] {
    let mut win = [false; 512];
    let mut mask = 0;
    while mask < 512 {
        let mut line = 0;
        while line < 8 {
            if mask as u16 & LINES[line] == LINES[line] {
                win[mask] = true;
            }
            line += 1;
        }
        mask += 1;
    }
    win
}

pub(crate) fn has_line(mask: u16) -> bool {
    WIN.get(usize::from(mask & FULL)).copied().unwrap_or(false)
}

#[cfg(test)]
#[allow(clippy::arithmetic_side_effects)]
mod tests {
    use super::*;

    /// Three marked Spaces in one row, column, or diagonal, found from coordinates alone.
    fn has_line_by_geometry(mask: u16) -> bool {
        let marked = |space: &Space| mask & space.bit() != 0;
        let lines: [fn(Space) -> bool; 8] = [
            |s| s.row() == 0,
            |s| s.row() == 1,
            |s| s.row() == 2,
            |s| s.col() == 0,
            |s| s.col() == 1,
            |s| s.col() == 2,
            |s| s.row() == s.col(),
            |s| s.row() + s.col() == 2,
        ];
        lines.iter().any(|on_line| {
            Space::ALL
                .iter()
                .filter(|&&space| on_line(space) && marked(&space))
                .count()
                == 3
        })
    }

    #[test]
    fn the_win_table_agrees_with_the_geometry() {
        for mask in 0..512u16 {
            assert_eq!(
                has_line(mask),
                has_line_by_geometry(mask),
                "mask {mask:09b}"
            );
        }
    }

    #[test]
    fn a_double_line_goes_to_the_mover() {
        let masks = Masks {
            x: 0b000_000_011,
            o: 0b011_000_000,
            drawn: 0b100_000_100,
        };
        assert_eq!(
            masks.settle(Player::O, DrawnOwner::Both),
            BoardStatus::Won(Player::O)
        );
        assert_eq!(
            masks.settle(Player::X, DrawnOwner::Both),
            BoardStatus::Won(Player::X)
        );
        assert_eq!(
            masks.settle(Player::X, DrawnOwner::Neither),
            BoardStatus::Open
        );
    }
}
