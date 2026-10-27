//! Rules data: the [`Ruleset`], its six settings, and the five [`Preset`]s. Plain data, valid by
//! construction.

use crate::error::ParseError;

/// The complete set of rule options a game is played under.
///
/// Six independent settings, and every combination is a legal game, so the struct has no
/// cross-field invariant. Each field's type is valid by construction, which is why the fields are
/// public: a Custom variant editor writes them directly.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Ruleset {
    pub depth: Depth,
    pub continuation: Continuation,
    pub drawn_owner: DrawnOwner,
    pub objective: Objective,
    pub fallback: Fallback,
    pub opening: Opening,
}

/// Levels of nesting. Two is 81 Cells in 9 Local boards; three is 729 Cells in 81 Local boards.
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(try_from = "u8", into = "u8")
)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[repr(u8)]
pub enum Depth {
    Two = 2,
    Three = 3,
}

impl Depth {
    /// The level of the Local boards, the Global board being level 0.
    pub(crate) const fn local_level(self) -> u8 {
        match self {
            Depth::Two => 1,
            Depth::Three => 2,
        }
    }
}

/// Whether Decided Boards take moves.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Continuation {
    /// A Decided Board takes no moves, and neither does anything beneath it.
    Off,
    /// Play continues inside Decided Boards of every size until they are full. Ownership never
    /// changes: the first line fixed it.
    On,
}

/// Who a drawn Board counts for.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum DrawnOwner {
    /// A drawn Board counts for nobody.
    Neither,
    /// A drawn Board counts toward both players' lines and counts. So one move can complete a
    /// line for the opponent, or for both players at once; a double line goes to the mover, at
    /// every level.
    Both,
}

/// What wins the game. Both objectives read only the Global board's children, and meeting the
/// objective is what winning the Global board means.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Objective {
    /// Three won children of the Global board in a row, column, or diagonal.
    Line,
    /// The first player with this many won children of the Global board. A line among them means
    /// nothing.
    FirstTo(Goal),
}

/// The N of `FirstTo(N)`, 1 to 9. Only `TryFrom<u8>` makes one.
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(try_from = "u8", into = "u8")
)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Goal(u8);

/// What decides a game whose Global board children are all Decided with the objective unmet.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Fallback {
    Draw,
    /// More won children of the Global board wins; equal counts draw.
    MostWon,
}

/// How the game's first moves are chosen.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Opening {
    /// X makes the first move, anywhere.
    Free,
    /// The first k plies are uniformly random legal moves that obey Sent, drawn from the game's
    /// Seed. `Game::new` plays them, and they are never in a Record's moves. Under `FirstTo(1)`
    /// with k of 5 or more the opening alone can end the game, which is allowed; the Custom
    /// variant editor warns.
    Random(RandomPlies),
}

/// The k of `Random(k)`, 1 to 8. Only `TryFrom<u8>` makes one.
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(try_from = "u8", into = "u8")
)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct RandomPlies(u8);

impl Goal {
    pub const MIN: u8 = 1;
    pub const MAX: u8 = 9;

    pub const fn get(self) -> u8 {
        self.0
    }
}

impl RandomPlies {
    pub const MIN: u8 = 1;
    pub const MAX: u8 = 8;

    pub const fn get(self) -> u8 {
        self.0
    }
}

impl TryFrom<u8> for Depth {
    type Error = ParseError;

    fn try_from(n: u8) -> Result<Depth, ParseError> {
        match n {
            2 => Ok(Depth::Two),
            3 => Ok(Depth::Three),
            _ => Err(ParseError::Depth(n)),
        }
    }
}

impl From<Depth> for u8 {
    fn from(depth: Depth) -> u8 {
        depth as u8
    }
}

impl TryFrom<u8> for Goal {
    type Error = ParseError;

    fn try_from(n: u8) -> Result<Goal, ParseError> {
        if (Goal::MIN..=Goal::MAX).contains(&n) {
            Ok(Goal(n))
        } else {
            Err(ParseError::Goal(n))
        }
    }
}

impl From<Goal> for u8 {
    fn from(goal: Goal) -> u8 {
        goal.0
    }
}

impl TryFrom<u8> for RandomPlies {
    type Error = ParseError;

    fn try_from(n: u8) -> Result<RandomPlies, ParseError> {
        if (RandomPlies::MIN..=RandomPlies::MAX).contains(&n) {
            Ok(RandomPlies(n))
        } else {
            Err(ParseError::RandomPlies(n))
        }
    }
}

impl From<RandomPlies> for u8 {
    fn from(plies: RandomPlies) -> u8 {
        plies.0
    }
}

/// The Variants that ship with Kokonoe, each a named Ruleset.
///
/// A Custom variant is any other Ruleset plus the name a player gave it. The engine plays,
/// validates, and replays Rulesets and never needs that name, so Custom variant names live with
/// the lobby and the Record format, not here.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Preset {
    Standard,
    ForcedContinuation,
    TicTacKu,
    CountTiebreak,
    Deep,
}

const STANDARD: Ruleset = Ruleset {
    depth: Depth::Two,
    continuation: Continuation::Off,
    drawn_owner: DrawnOwner::Neither,
    objective: Objective::Line,
    fallback: Fallback::Draw,
    opening: Opening::Free,
};

impl Preset {
    pub const ALL: [Preset; 5] = [
        Preset::Standard,
        Preset::ForcedContinuation,
        Preset::TicTacKu,
        Preset::CountTiebreak,
        Preset::Deep,
    ];

    /// This table is the definition of the Presets.
    pub const fn ruleset(self) -> Ruleset {
        match self {
            Preset::Standard => STANDARD,
            Preset::ForcedContinuation => Ruleset {
                continuation: Continuation::On,
                ..STANDARD
            },
            Preset::TicTacKu => Ruleset {
                continuation: Continuation::On,
                objective: Objective::FirstTo(Goal(5)),
                fallback: Fallback::MostWon,
                ..STANDARD
            },
            Preset::CountTiebreak => Ruleset {
                fallback: Fallback::MostWon,
                ..STANDARD
            },
            Preset::Deep => Ruleset {
                depth: Depth::Three,
                ..STANDARD
            },
        }
    }

    /// The Preset whose Ruleset equals `ruleset`. A Custom variant that happens to match Standard
    /// is Standard for lobby queues and ratings.
    pub fn of(ruleset: &Ruleset) -> Option<Preset> {
        Preset::ALL
            .into_iter()
            .find(|preset| preset.ruleset() == *ruleset)
    }

    pub const fn name(self) -> &'static str {
        match self {
            Preset::Standard => "Standard",
            Preset::ForcedContinuation => "Forced continuation",
            Preset::TicTacKu => "Tic-Tac-Ku",
            Preset::CountTiebreak => "Count tiebreak",
            Preset::Deep => "Deep",
        }
    }
}
