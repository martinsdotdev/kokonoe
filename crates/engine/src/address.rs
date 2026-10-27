//! Addresses: [`Space`], [`Cell`], [`Board`]. One recursive scheme, identical at depth 2 and 3.
//!
//! An address is a path of Spaces from the Global board down. The Global board is the empty path,
//! a Local board is a path of D-1 Spaces, and a Cell is a path of D Spaces. Boards between the two
//! (depth 3 only) need no type or name of their own: they are Boards whose children are Boards.
//! Sent drops the first Space of a Cell's path, and relaxation walks [`Board::parent`].

use core::ops::{Deref, Range};

use crate::error::ParseError;
use crate::ruleset::Depth;

/// One of the nine spaces of a Board, row-major: `0 1 2 / 3 4 5 / 6 7 8`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Space(u8);

impl Space {
    pub const ALL: [Space; 9] = [
        Space(0),
        Space(1),
        Space(2),
        Space(3),
        Space(4),
        Space(5),
        Space(6),
        Space(7),
        Space(8),
    ];

    pub const fn index(self) -> u8 {
        self.0
    }

    pub const fn row(self) -> u8 {
        self.0 / 3
    }

    pub const fn col(self) -> u8 {
        self.0 % 3
    }

    /// This Space's bit in a Board's 9-bit masks.
    pub(crate) const fn bit(self) -> u16 {
        1 << self.0
    }
}

impl TryFrom<u8> for Space {
    type Error = ParseError;

    fn try_from(n: u8) -> Result<Space, ParseError> {
        if n < 9 {
            Ok(Space(n))
        } else {
            Err(ParseError::Space(n))
        }
    }
}

/// A path of at most three Spaces, read from the Global board down.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Spaces {
    len: u8,
    path: [Space; 3],
}

impl Spaces {
    /// The `len` base-9 digits of `index`, most significant first.
    fn digits(mut index: u16, len: u8) -> Spaces {
        let mut path = [Space(0); 3];
        for space in path.iter_mut().take(usize::from(len)).rev() {
            *space = Space((index % 9) as u8);
            index /= 9;
        }
        Spaces { len, path }
    }
}

impl Deref for Spaces {
    type Target = [Space];

    fn deref(&self) -> &[Space] {
        self.path.get(..usize::from(self.len)).unwrap_or(&[])
    }
}

/// `9^n` for the exponents an address needs, 0 to 3.
const fn pow9(n: u8) -> u16 {
    match n {
        0 => 1,
        1 => 9,
        2 => 81,
        _ => 729,
    }
}

/// Bit 15 of a [`Cell`]: set for a Cell of a depth-3 game.
const DEPTH_THREE: u16 = 1 << 15;

/// The address of one Cell: the Spaces `d1..dD` from the Global board down, D being its depth.
///
/// Two bytes. Bit 15 marks depth 3, and bits 0 to 9 hold `d1..dD` read as a base-9 number, so a
/// search tree stores one per node. `Ord` is address order (`d1` first), the order
/// [`LegalMoves`](crate::LegalMoves) iterates in.
///
/// Invariant: in range for its own depth. Only [`Cell::from_digits`], [`Cell::from_index`],
/// [`Board::child`], and `LegalMoves` make Cells.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Cell(u16);

impl Cell {
    /// `index` must be below `9^depth`.
    pub(crate) const fn new(depth: Depth, index: u16) -> Cell {
        match depth {
            Depth::Two => Cell(index),
            Depth::Three => Cell(index | DEPTH_THREE),
        }
    }

    /// The parse boundary for an untrusted move: two or three digits, each 0 to 8. The digit
    /// count is the depth, so a well-formed Cell can still belong to the other depth; `play`
    /// rejects that as [`IllegalMove::WrongDepth`](crate::IllegalMove::WrongDepth).
    pub fn from_digits(digits: &[u8]) -> Result<Cell, ParseError> {
        let depth = match digits.len() {
            2 => Depth::Two,
            3 => Depth::Three,
            n => return Err(ParseError::AddressLength(n)),
        };
        let mut index: u16 = 0;
        for &digit in digits {
            let space = Space::try_from(digit)?;
            // At most 728 after the third digit.
            index = index.wrapping_mul(9).wrapping_add(u16::from(space.0));
        }
        Ok(Cell::new(depth, index))
    }

    /// The parse boundary for a move on the wire: its base-9 index (see [`Cell::index`]), read
    /// under the game's depth.
    pub fn from_index(depth: Depth, index: u16) -> Result<Cell, ParseError> {
        if index < pow9(depth as u8) {
            Ok(Cell::new(depth, index))
        } else {
            Err(ParseError::CellIndex(index))
        }
    }

    pub const fn depth(self) -> Depth {
        if self.0 & DEPTH_THREE == 0 {
            Depth::Two
        } else {
            Depth::Three
        }
    }

    /// The base-9 path as a number, below `9^depth`, with `d1` the most significant digit: the
    /// form a move takes on the wire and in storage. Also this Cell's bit in a LegalMoves.
    pub const fn index(self) -> u16 {
        self.0 & !DEPTH_THREE
    }

    /// The last Space of the path: where this Cell sits on its Local board.
    pub(crate) const fn space(self) -> Space {
        Space((self.index() % 9) as u8)
    }

    pub fn spaces(self) -> Spaces {
        Spaces::digits(self.index(), self.depth() as u8)
    }

    /// The Local board this Cell lies on: its address without the last Space.
    pub fn local_board(self) -> Board {
        let depth = self.depth();
        Board {
            depth,
            level: depth.local_level(),
            index: (self.index() / 9) as u8,
        }
    }

    /// Sent: the Local board this move sends the opponent to, `d2..dD`, which is the address
    /// without its first Space. One rule at every depth. At depth 2 it reads "Cell k sends to
    /// Local board k"; in index terms it is `index mod 9^(D-1)`. The UI calls it on hover to
    /// preview where a move sends the opponent.
    pub fn sends_to(self) -> Board {
        let depth = self.depth();
        let index = match depth {
            Depth::Two => self.index() % 9,
            Depth::Three => self.index() % 81,
        };
        Board {
            depth,
            level: depth.local_level(),
            index: index as u8,
        }
    }
}

/// The address of one Board: the Global board, a Local board, or one nested between them.
///
/// Invariant: `level < depth`. Level 0 is the Global board and level `depth - 1` is a Local
/// board; `index` is the path read as a base-9 number, below `9^level`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Board {
    depth: Depth,
    level: u8,
    index: u8,
}

impl Board {
    pub const fn global(depth: Depth) -> Board {
        Board {
            depth,
            level: 0,
            index: 0,
        }
    }

    /// The parse boundary for a Board on the wire: its Space path from the Global board down,
    /// empty for the Global board itself, read under the game's depth.
    pub fn from_path(depth: Depth, path: &[u8]) -> Result<Board, ParseError> {
        let mut board = Board::global(depth);
        for &space in path {
            board = match board.child(Space::try_from(space)?) {
                Child::Board(child) => child,
                Child::Cell(_) => return Err(ParseError::BoardPath(path.len())),
            };
        }
        Ok(board)
    }

    pub const fn depth(self) -> Depth {
        self.depth
    }

    /// The path from the Global board, empty for the Global board itself.
    pub fn spaces(self) -> Spaces {
        Spaces::digits(u16::from(self.index), self.level)
    }

    /// `None` for the Global board. Relaxation walks this.
    pub fn parent(self) -> Option<Board> {
        let level = self.level.checked_sub(1)?;
        Some(Board {
            depth: self.depth,
            level,
            index: self.index / 9,
        })
    }

    /// What sits at `space`: a smaller Board, or a Cell when this is a Local board. Rendering
    /// recurses on this and never asks for the depth.
    pub fn child(self, space: Space) -> Child {
        if self.is_local() {
            // At most 80 · 9 + 8 = 728.
            let index = u16::from(self.index)
                .wrapping_mul(9)
                .wrapping_add(u16::from(space.0));
            Child::Cell(Cell::new(self.depth, index))
        } else {
            // A Board above the Local boards is at most level 1, index 8, so this is at most 80.
            Child::Board(Board {
                depth: self.depth,
                level: self.level.wrapping_add(1),
                index: self.index.wrapping_mul(9).wrapping_add(space.0),
            })
        }
    }

    /// Whether this Board's spaces are Cells.
    pub const fn is_local(self) -> bool {
        self.level == self.depth.local_level()
    }

    pub(crate) const fn is_global(self) -> bool {
        self.level == 0
    }

    /// Where this Board sits in its parent. Meaningless for the Global board.
    pub(crate) const fn space(self) -> Space {
        Space(self.index % 9)
    }

    /// This Board's place in breadth-first order: 0 is the Global board, 1 to 9 its children,
    /// and 10 to 90 theirs at depth 3.
    pub(crate) const fn slot(self) -> usize {
        let first: usize = match self.level {
            0 => 0,
            1 => 1,
            _ => 10,
        };
        // At most 10 + 80.
        first.wrapping_add(self.index as usize)
    }

    /// The indices of the Cells inside this Board, a contiguous run in address order.
    pub(crate) const fn cells(self) -> Range<u16> {
        let span = pow9((self.depth as u8).wrapping_sub(self.level));
        // At most 80 · 9 and 81 · 9.
        let start = (self.index as u16).wrapping_mul(span);
        start..start.wrapping_add(span)
    }
}

/// What a Board holds at one Space.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Child {
    Board(Board),
    Cell(Cell),
}
