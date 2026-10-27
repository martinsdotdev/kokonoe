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
    /// Not 1 to 8.
    Level(u8),
    /// Not Notation: digits 1 to 9 joined by `·` or `.`.
    Notation,
    /// Not a Ruleset code, for the reason given.
    RulesetCode(CodeError),
    /// Not 16 lowercase hex digits.
    Seed,
    /// A Board path of this many Spaces names a Cell or runs past the Local boards.
    BoardPath(usize),
}

/// Why a Ruleset code was refused. Each code has one spelling, so these are the ways to miss it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CodeError {
    /// Nothing between two `+`, or at either end.
    Empty,
    /// Not one of `std`, `d3`, `cont`, `both`, `first<N>`, `most` and `rand<K>`.
    Unknown,
    /// `std` together with other settings.
    StdCombined,
    /// A setting after one that comes later in `d3`, `cont`, `both`, `first`, `most`, `rand`.
    OutOfOrder,
    /// The same setting twice.
    Duplicate,
    /// A number with a leading zero.
    LeadingZero,
}

impl From<CodeError> for ParseError {
    fn from(error: CodeError) -> ParseError {
        ParseError::RulesetCode(error)
    }
}

impl fmt::Display for CodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            CodeError::Empty => "a Ruleset code has an empty setting",
            CodeError::Unknown => {
                "a Ruleset code's settings are std, d3, cont, both, first<N>, most and rand<K>"
            }
            CodeError::StdCombined => "std stands alone in a Ruleset code",
            CodeError::OutOfOrder => {
                "a Ruleset code's settings go in the order d3, cont, both, first, most, rand"
            }
            CodeError::Duplicate => "a Ruleset code names a setting twice",
            CodeError::LeadingZero => "a Ruleset code's numbers have no leading zero",
        })
    }
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
            ParseError::Level(n) => write!(f, "Level {n} is outside 1 to 8"),
            ParseError::Notation => f.write_str("Notation is digits 1 to 9 joined by · or ."),
            ParseError::RulesetCode(error) => error.fmt(f),
            ParseError::Seed => f.write_str("a Seed is 16 lowercase hex digits"),
            ParseError::BoardPath(n) => write!(f, "a Board path of {n} Spaces names no Board"),
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

/// Why [`Game::open`](crate::Game::open) refused a Record.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OpenError {
    /// The Record was played under this `RULES_VERSION`, not this build's.
    RulesVersion(u32),
    /// A move of the Record the rules refuse.
    Illegal(ReplayError),
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

impl fmt::Display for OpenError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            OpenError::RulesVersion(n) => write!(
                f,
                "the Record is from rules version {n}, and this build plays version {}",
                crate::RULES_VERSION
            ),
            OpenError::Illegal(error) => error.fmt(f),
        }
    }
}

impl core::error::Error for IllegalMove {}
impl core::error::Error for OpenError {}
impl core::error::Error for PlayError {}
impl core::error::Error for ReplayError {}
