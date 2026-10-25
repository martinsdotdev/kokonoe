//! Errors in two layers: [`ParseError`] for data with the wrong shape, then the rules' refusals.

use core::fmt;

use crate::address::Board;
use crate::play::Ply;

/// Data with the wrong shape: a value out of range before any rule is consulted.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ParseError {
    /// Not 2 or 3.
    Depth(u8),
    /// Not 0 to 8.
    Space(u8),
    /// An address with a digit count other than 2 or 3.
    AddressLength(usize),
    /// A Cell index of `9^depth` or more.
    CellIndex(u16),
    /// Not 1 to 9.
    Goal(u8),
    /// Not 1 to 8.
    RandomPlies(u8),
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ParseError::Depth(n) => write!(f, "depth {n} is not 2 or 3"),
            ParseError::Space(n) => write!(f, "space {n} is not 0 to 8"),
            ParseError::AddressLength(n) => write!(f, "an address has 2 or 3 spaces, not {n}"),
            ParseError::CellIndex(n) => write!(f, "cell index {n} is past the last Cell"),
            ParseError::Goal(n) => write!(f, "FirstTo({n}) is outside 1 to 9"),
            ParseError::RandomPlies(n) => write!(f, "Random({n}) is outside 1 to 8"),
        }
    }
}

impl core::error::Error for ParseError {}

/// A well-formed Cell the rules don't allow now. Each variant names the Board the UI should
/// flash while it explains.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IllegalMove {
    GameOver,
    /// The Cell belongs to a game of the other depth.
    WrongDepth,
    Occupied,
    /// Continuation is off and `board`, which contains the Cell, is Decided. The outermost such
    /// Board.
    BeneathDecided {
        board: Board,
    },
    /// The Cell is outside `within`, the Board the player may play inside.
    OutsideActiveBoards {
        within: Board,
    },
}

/// Why [`Game::play`](crate::Game::play) refused. Kept apart because the UI handles them
/// differently: an illegal move is explained to the player, a wrong ply is resynchronised with
/// the move authority.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlayError {
    Illegal(IllegalMove),
    OutOfOrder { want: Ply, got: Ply },
}

/// The first move of a Record that the rules reject, as an index into the Record's moves.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ReplayError {
    pub index: usize,
    pub error: IllegalMove,
}

impl From<IllegalMove> for PlayError {
    fn from(error: IllegalMove) -> PlayError {
        PlayError::Illegal(error)
    }
}

impl fmt::Display for IllegalMove {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            IllegalMove::GameOver => "the game is over",
            IllegalMove::WrongDepth => "the Cell belongs to a game of the other depth",
            IllegalMove::Occupied => "the Cell is already marked",
            IllegalMove::BeneathDecided { .. } => "the Cell is inside a Decided Board",
            IllegalMove::OutsideActiveBoards { .. } => "the Cell is outside the Active boards",
        })
    }
}

impl fmt::Display for PlayError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PlayError::Illegal(error) => error.fmt(f),
            PlayError::OutOfOrder { want, got } => write!(
                f,
                "move stamped ply {} but the game is at ply {}",
                got.get(),
                want.get()
            ),
        }
    }
}

impl fmt::Display for ReplayError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "move {} is illegal: {}", self.index, self.error)
    }
}

impl core::error::Error for IllegalMove {}
impl core::error::Error for PlayError {}
impl core::error::Error for ReplayError {}
