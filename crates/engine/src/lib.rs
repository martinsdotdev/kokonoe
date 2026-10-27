//! The rules of Kokonoe: Rulesets, play, and Records, and the computer opponent's search.
//!
//! Pure and `no_std`, so the same crate runs natively on the server and as WASM in the
//! browser and the AI Worker.
//!
//! Contract for every public function: it never panics, whatever the input. Untrusted data enters
//! through `TryFrom`, then `Game::open` and `Game::play`. The lints below hold the code to it.

#![no_std]
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects
)]

extern crate alloc;

mod address;
mod error;
mod play;
mod repr;
mod rng;
mod ruleset;
mod search;

pub use address::{Board, Cell, Child, Space, Spaces};
pub use error::{IllegalMove, ParseError, PlayError, ReplayError};
pub use play::{
    BoardStatus, Game, LegalIter, LegalMoves, Outcome, Played, Player, Ply, Record, Seed, Sent,
    State, Status,
};
pub use ruleset::{
    Continuation, Depth, DrawnOwner, Fallback, Goal, Objective, Opening, Preset, RandomPlies,
    Ruleset,
};
pub use search::{Choice, Level, think};

/// Bumped whenever the same Record could replay to a different [`State`]: a rules fix, a new
/// opening generator, or a new legal-move order (`tests/openings.rs` pins both). It is stored
/// beside every serialized Record, so a client and a server on different releases detect the
/// skew instead of silently disagreeing about a game.
pub const RULES_VERSION: u32 = 1;
