//! The engine's values as the mirrors in [`crate::types`], and back.
//!
//! Every conversion matches exhaustively, both ways, so a variant added to the engine doesn't
//! compile until `types.rs`, and with it the generated `.d.ts`, has it too. Conversions into the
//! engine are the parse boundary: each yields a valid engine value or says what is [`Malformed`].

use core::fmt;

use kokonoe_engine as engine;

use crate::types::{
    Board, BoardStatus, Cell, Continuation, DrawnOwner, Fallback, IllegalMove, Objective, Opening,
    Outcome, PlayResult, Player, Ply, Preset, Record, ReplayError, Ruleset, Seed, Sent, Status,
};

/// Data the engine can't even consider: wrong in shape, before any rule is consulted.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Malformed {
    /// Not the declared shape. Names what was expected.
    Shape(&'static str),
    /// A value outside the engine's range: a depth, a Space, a Goal, a Random k or a Cell index.
    Parse(engine::ParseError),
    /// A Board path that doesn't name a Board of the game: a Cell's path or longer.
    BoardPath(usize),
    /// A Record from another rules version than this build plays.
    RulesVersion(u32),
}

impl From<engine::ParseError> for Malformed {
    fn from(error: engine::ParseError) -> Malformed {
        Malformed::Parse(error)
    }
}

impl fmt::Display for Malformed {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Malformed::Shape(expected) => f.write_str(expected),
            Malformed::Parse(error) => write!(f, "{error}"),
            Malformed::BoardPath(n) => write!(f, "a Board path of {n} Spaces names no Board"),
            Malformed::RulesVersion(n) => write!(
                f,
                "the Record is from rules version {n}, and this build plays version {}",
                engine::RULES_VERSION
            ),
        }
    }
}

impl std::error::Error for Malformed {}

impl From<engine::Player> for Player {
    fn from(player: engine::Player) -> Player {
        match player {
            engine::Player::X => Player::X,
            engine::Player::O => Player::O,
        }
    }
}

impl From<Player> for engine::Player {
    fn from(player: Player) -> engine::Player {
        match player {
            Player::X => engine::Player::X,
            Player::O => engine::Player::O,
        }
    }
}

impl From<engine::Cell> for Cell {
    fn from(cell: engine::Cell) -> Cell {
        Cell(cell.index())
    }
}

impl Cell {
    /// This index as a Cell of a game of `depth`.
    pub fn at_depth(self, depth: engine::Depth) -> Result<engine::Cell, Malformed> {
        Ok(engine::Cell::from_index(depth, self.0)?)
    }
}

impl From<engine::Board> for Board {
    fn from(board: engine::Board) -> Board {
        Board(board.spaces().iter().map(|space| space.index()).collect())
    }
}

impl Board {
    /// The Board at this path in a game of `depth`, read from the Global board down.
    pub fn at_depth(&self, depth: engine::Depth) -> Result<engine::Board, Malformed> {
        let mut board = engine::Board::global(depth);
        for &space in &self.0 {
            board = match board.child(engine::Space::try_from(space)?) {
                engine::Child::Board(child) => child,
                engine::Child::Cell(_) => return Err(Malformed::BoardPath(self.0.len())),
            };
        }
        Ok(board)
    }
}

impl From<engine::Ply> for Ply {
    fn from(ply: engine::Ply) -> Ply {
        Ply(ply.get().into())
    }
}

impl Ply {
    /// `None` past the engine's range, which no game reaches.
    pub fn in_range(self) -> Option<engine::Ply> {
        u16::try_from(self.0).ok().map(engine::Ply::new)
    }
}

impl From<engine::Seed> for Seed {
    fn from(seed: engine::Seed) -> Seed {
        let mut hex = String::with_capacity(16);
        for byte in seed.0.to_be_bytes() {
            for nibble in [byte >> 4, byte & 0xf] {
                hex.push(char::from_digit(u32::from(nibble), 16).unwrap_or('0'));
            }
        }
        Seed(hex)
    }
}

impl TryFrom<&Seed> for engine::Seed {
    type Error = Malformed;

    /// Exactly 16 lowercase hex digits, the one spelling `From` writes, so a Record has one
    /// encoding.
    fn try_from(seed: &Seed) -> Result<engine::Seed, Malformed> {
        let hex = seed.0.as_str();
        // from_str_radix alone would also take a sign and uppercase.
        if hex.len() != 16 || !hex.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f')) {
            return Err(Malformed::Shape("a Seed is 16 lowercase hex digits"));
        }
        u64::from_str_radix(hex, 16)
            .map(engine::Seed)
            .map_err(|_| Malformed::Shape("a Seed is 16 lowercase hex digits"))
    }
}

impl From<engine::Ruleset> for Ruleset {
    fn from(rules: engine::Ruleset) -> Ruleset {
        Ruleset {
            depth: rules.depth.into(),
            continuation: match rules.continuation {
                engine::Continuation::Off => Continuation::Off,
                engine::Continuation::On => Continuation::On,
            },
            drawn_owner: match rules.drawn_owner {
                engine::DrawnOwner::Neither => DrawnOwner::Neither,
                engine::DrawnOwner::Both => DrawnOwner::Both,
            },
            objective: match rules.objective {
                engine::Objective::Line => Objective::Line,
                engine::Objective::FirstTo(goal) => Objective::FirstTo(goal.get()),
            },
            fallback: match rules.fallback {
                engine::Fallback::Draw => Fallback::Draw,
                engine::Fallback::MostWon => Fallback::MostWon,
            },
            opening: match rules.opening {
                engine::Opening::Free => Opening::Free,
                engine::Opening::Random(k) => Opening::Random(k.get()),
            },
        }
    }
}

impl TryFrom<Ruleset> for engine::Ruleset {
    type Error = Malformed;

    fn try_from(rules: Ruleset) -> Result<engine::Ruleset, Malformed> {
        Ok(engine::Ruleset {
            depth: engine::Depth::try_from(rules.depth)?,
            continuation: match rules.continuation {
                Continuation::Off => engine::Continuation::Off,
                Continuation::On => engine::Continuation::On,
            },
            drawn_owner: match rules.drawn_owner {
                DrawnOwner::Neither => engine::DrawnOwner::Neither,
                DrawnOwner::Both => engine::DrawnOwner::Both,
            },
            objective: match rules.objective {
                Objective::Line => engine::Objective::Line,
                Objective::FirstTo(n) => engine::Objective::FirstTo(engine::Goal::try_from(n)?),
            },
            fallback: match rules.fallback {
                Fallback::Draw => engine::Fallback::Draw,
                Fallback::MostWon => engine::Fallback::MostWon,
            },
            opening: match rules.opening {
                Opening::Free => engine::Opening::Free,
                Opening::Random(k) => engine::Opening::Random(engine::RandomPlies::try_from(k)?),
            },
        })
    }
}

impl From<engine::Record> for Record {
    fn from(record: engine::Record) -> Record {
        Record {
            rules_version: engine::RULES_VERSION,
            ruleset: record.ruleset.into(),
            seed: record.seed.into(),
            moves: record.moves.into_iter().map(Cell::from).collect(),
        }
    }
}

impl TryFrom<Record> for engine::Record {
    type Error = Malformed;

    fn try_from(record: Record) -> Result<engine::Record, Malformed> {
        if record.rules_version != engine::RULES_VERSION {
            return Err(Malformed::RulesVersion(record.rules_version));
        }
        let ruleset = engine::Ruleset::try_from(record.ruleset)?;
        Ok(engine::Record {
            ruleset,
            seed: engine::Seed::try_from(&record.seed)?,
            moves: record
                .moves
                .into_iter()
                .map(|cell| cell.at_depth(ruleset.depth))
                .collect::<Result<_, _>>()?,
        })
    }
}

impl From<engine::Status> for Status {
    fn from(status: engine::Status) -> Status {
        match status {
            engine::Status::ToMove {
                player,
                sent,
                moves,
            } => Status::ToMove {
                player: player.into(),
                sent: sent.into(),
                moves: moves.iter().map(Cell::from).collect(),
            },
            engine::Status::Over(outcome) => Status::Over(outcome.into()),
        }
    }
}

impl From<engine::Sent> for Sent {
    fn from(sent: engine::Sent) -> Sent {
        match sent {
            engine::Sent::FirstMove => Sent::FirstMove,
            engine::Sent::To(board) => Sent::To(board.into()),
            engine::Sent::Free { target, within } => Sent::Free {
                target: target.into(),
                within: within.into(),
            },
        }
    }
}

impl From<engine::Outcome> for Outcome {
    fn from(outcome: engine::Outcome) -> Outcome {
        match outcome {
            engine::Outcome::Objective(player) => Outcome::Objective(player.into()),
            engine::Outcome::MostWon(player) => Outcome::MostWon(player.into()),
            engine::Outcome::Draw => Outcome::Draw,
        }
    }
}

impl From<engine::BoardStatus> for BoardStatus {
    fn from(status: engine::BoardStatus) -> BoardStatus {
        match status {
            engine::BoardStatus::Open => BoardStatus::Open,
            engine::BoardStatus::Won(player) => BoardStatus::Won(player.into()),
            engine::BoardStatus::Drawn => BoardStatus::Drawn,
        }
    }
}

impl From<engine::IllegalMove> for IllegalMove {
    fn from(error: engine::IllegalMove) -> IllegalMove {
        match error {
            engine::IllegalMove::GameOver => IllegalMove::GameOver,
            engine::IllegalMove::WrongDepth => IllegalMove::WrongDepth,
            engine::IllegalMove::Occupied => IllegalMove::Occupied,
            engine::IllegalMove::BeneathDecided { board } => IllegalMove::BeneathDecided {
                board: board.into(),
            },
            engine::IllegalMove::OutsideActiveBoards { within } => {
                IllegalMove::OutsideActiveBoards {
                    within: within.into(),
                }
            }
        }
    }
}

impl From<Result<engine::Played, engine::PlayError>> for PlayResult {
    fn from(result: Result<engine::Played, engine::PlayError>) -> PlayResult {
        match result {
            Ok(engine::Played::Now(status)) => PlayResult::Now(status.into()),
            Ok(engine::Played::Already) => PlayResult::Already,
            Err(engine::PlayError::Illegal(error)) => PlayResult::Illegal(error.into()),
            Err(engine::PlayError::OutOfOrder { want, got }) => PlayResult::OutOfOrder {
                want: want.into(),
                got: got.into(),
            },
        }
    }
}

impl From<engine::ReplayError> for ReplayError {
    fn from(error: engine::ReplayError) -> ReplayError {
        ReplayError {
            // A Record's moves fit a game's 729 Cells, so this never saturates.
            index: u32::try_from(error.index).unwrap_or(u32::MAX),
            error: error.error.into(),
        }
    }
}

impl From<engine::Preset> for Preset {
    fn from(preset: engine::Preset) -> Preset {
        Preset {
            name: preset.name().into(),
            ruleset: preset.ruleset().into(),
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::arithmetic_side_effects)]
mod tests {
    use super::*;

    #[test]
    fn every_ruleset_round_trips() {
        let mut count = 0;
        for depth in [2, 3] {
            for continuation in [Continuation::Off, Continuation::On] {
                for drawn_owner in [DrawnOwner::Neither, DrawnOwner::Both] {
                    let objectives = (1..=9).map(Objective::FirstTo);
                    for objective in [Objective::Line].into_iter().chain(objectives) {
                        for fallback in [Fallback::Draw, Fallback::MostWon] {
                            let openings = (1..=8).map(Opening::Random);
                            for opening in [Opening::Free].into_iter().chain(openings) {
                                let rules = Ruleset {
                                    depth,
                                    continuation,
                                    drawn_owner,
                                    objective,
                                    fallback,
                                    opening,
                                };
                                let engine = engine::Ruleset::try_from(rules).unwrap();
                                assert_eq!(Ruleset::from(engine), rules);
                                count += 1;
                            }
                        }
                    }
                }
            }
        }
        assert_eq!(count, 1440);
    }

    #[test]
    fn rulesets_out_of_range_are_malformed() {
        let standard = Ruleset::from(engine::Preset::Standard.ruleset());
        let cases = [
            (
                Ruleset {
                    depth: 4,
                    ..standard
                },
                engine::ParseError::Depth(4),
            ),
            (
                Ruleset {
                    objective: Objective::FirstTo(10),
                    ..standard
                },
                engine::ParseError::Goal(10),
            ),
            (
                Ruleset {
                    opening: Opening::Random(0),
                    ..standard
                },
                engine::ParseError::RandomPlies(0),
            ),
        ];
        for (rules, error) in cases {
            assert_eq!(
                engine::Ruleset::try_from(rules),
                Err(Malformed::Parse(error))
            );
        }
    }

    #[test]
    fn cells_and_boards_round_trip_at_both_depths() {
        for (depth, count) in [(engine::Depth::Two, 81), (engine::Depth::Three, 729)] {
            for index in 0..count {
                let cell = engine::Cell::from_index(depth, index).unwrap();
                assert_eq!(Cell::from(cell).at_depth(depth), Ok(cell));
                let mut board = Some(cell.local_board());
                while let Some(b) = board {
                    assert_eq!(Board::from(b).at_depth(depth), Ok(b));
                    board = b.parent();
                }
            }
            assert_eq!(
                Cell(count).at_depth(depth),
                Err(Malformed::Parse(engine::ParseError::CellIndex(count)))
            );
        }
        assert_eq!(
            Board(vec![4, 2]).at_depth(engine::Depth::Two),
            Err(Malformed::BoardPath(2)),
            "a Cell's path"
        );
        assert_eq!(
            Board(vec![9]).at_depth(engine::Depth::Two),
            Err(Malformed::Parse(engine::ParseError::Space(9)))
        );
    }

    #[test]
    fn plies_past_the_engine_are_out_of_range() {
        assert_eq!(Ply(65_535).in_range(), Some(engine::Ply::new(65_535)));
        assert_eq!(Ply(65_536).in_range(), None);
    }

    #[test]
    fn seeds_are_16_lowercase_hex_digits() {
        for n in [0, 0x5eed00, (1 << 53) + 1, u64::MAX] {
            let seed = Seed::from(engine::Seed(n));
            assert_eq!(seed.0.len(), 16);
            assert_eq!(engine::Seed::try_from(&seed), Ok(engine::Seed(n)));
        }
        assert_eq!(
            Seed::from(engine::Seed(0x5eed00)).0,
            "00000000005eed00",
            "spec §4.3's example"
        );
        let bad = [
            "00000000005EED00",
            "5eed",
            "+000000000000000",
            "g000000000000000",
            "00000000000000000",
        ];
        for hex in bad {
            assert!(engine::Seed::try_from(&Seed(hex.into())).is_err(), "{hex}");
        }
    }

    #[test]
    fn records_check_the_rules_version() {
        let mut game =
            engine::Game::new(engine::Preset::Standard.ruleset(), engine::Seed(0x5eed00));
        let at = game.state().ply();
        game.play(at, engine::Cell::from_digits(&[4, 2]).unwrap())
            .unwrap();
        let record = Record::from(game.record());
        assert_eq!(record.moves, [Cell(38)]);
        assert_eq!(engine::Record::try_from(record.clone()), Ok(game.record()));

        let future = Record {
            rules_version: engine::RULES_VERSION + 1,
            ..record
        };
        assert_eq!(
            engine::Record::try_from(future),
            Err(Malformed::RulesVersion(engine::RULES_VERSION + 1))
        );
    }
}
