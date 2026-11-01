//! The engine's values as the mirrors in [`crate::types`], and back.
//!
//! Every conversion matches exhaustively, both ways, so a variant added to the engine doesn't
//! compile until `types.rs`, and with it the generated `.d.ts`, has it too. Conversions into the
//! engine are the parse boundary: each yields a valid engine value or says what is [`Malformed`].

use core::fmt;

use kokonoe_engine as engine;
use kokonoe_protocol as protocol;

use crate::types::{
    Board, BoardStatus, Cell, Continuation, DrawnOwner, Fallback, IllegalMove, Objective, Opening,
    Outcome, PlayResult, Player, Ply, Preset, Record, ReplayError, Ruleset, Searched, Seed, Sent,
    Status, Visit,
};

/// Data the engine can't even consider: wrong in shape, before any rule is consulted.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Malformed {
    /// Not the declared shape. Names what was expected.
    Shape(&'static str),
    /// A value the engine's parsers refuse: out of range, or not the one spelling it reads.
    Parse(engine::ParseError),
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
        Ok(engine::Board::from_path(depth, &self.0)?)
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
        Seed(seed.to_string())
    }
}

impl TryFrom<&Seed> for engine::Seed {
    type Error = Malformed;

    /// Exactly 16 lowercase hex digits, the one spelling a Record writes.
    fn try_from(seed: &Seed) -> Result<engine::Seed, Malformed> {
        Ok(seed.0.parse()?)
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
            rules_version: record.rules_version,
            ruleset: record.ruleset.into(),
            seed: record.seed.into(),
            moves: record.moves.into_iter().map(Cell::from).collect(),
        }
    }
}

impl TryFrom<Record> for engine::Record {
    type Error = Malformed;

    /// The Record as written. `Game::open` is the one that refuses another rules version.
    fn try_from(record: Record) -> Result<engine::Record, Malformed> {
        let ruleset = engine::Ruleset::try_from(record.ruleset)?;
        Ok(engine::Record {
            rules_version: record.rules_version,
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

impl From<engine::Searched> for Searched {
    fn from(searched: engine::Searched) -> Searched {
        match searched {
            engine::Searched::Forced(cell) => Searched::Forced(cell.into()),
            engine::Searched::Visits(visits) => Searched::Visits(
                visits
                    .into_iter()
                    .map(|(cell, count)| Visit {
                        cell: cell.into(),
                        count,
                    })
                    .collect(),
            ),
        }
    }
}

impl Visit {
    /// This visit as one of a game of `depth`'s root moves.
    pub fn at_depth(self, depth: engine::Depth) -> Result<(engine::Cell, u32), Malformed> {
        Ok((self.cell.at_depth(depth)?, self.count))
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

impl From<protocol::CloseCode> for crate::CloseCode {
    fn from(close: protocol::CloseCode) -> crate::CloseCode {
        match close {
            protocol::CloseCode::Restarting => crate::CloseCode::Restarting,
            protocol::CloseCode::UpdateRequired => crate::CloseCode::UpdateRequired,
            protocol::CloseCode::Forbidden => crate::CloseCode::Forbidden,
            protocol::CloseCode::NoRoom => crate::CloseCode::NoRoom,
        }
    }
}

/// The Room's messages and routes as their mirrors, for the tests that hold the two to the same
/// JSON. Exhaustive like the rest, so a message the protocol adds doesn't compile until
/// `wire.rs` declares it.
#[cfg(test)]
mod wire {
    use super::*;
    use crate::wire::{
        ClientMsg, CreatedRoom, Draws, GameEnd, GameResult, NewRoom, Offer, OfferKind, Players,
        Rejection, Role, RoomCard, SeatChoice, ServerMsg, Snapshot, Termination,
    };

    impl From<protocol::ClientMsg> for ClientMsg {
        fn from(message: protocol::ClientMsg) -> ClientMsg {
            match message {
                protocol::ClientMsg::Hello {
                    protocol,
                    rules_version,
                    room,
                    role,
                } => ClientMsg::Hello {
                    protocol,
                    rules_version,
                    room,
                    role: role.into(),
                },
                protocol::ClientMsg::Move { ply, cell } => ClientMsg::Move {
                    ply: Ply(ply.into()),
                    cell: Cell(cell),
                },
                protocol::ClientMsg::Resign => ClientMsg::Resign,
                protocol::ClientMsg::OfferDraw => ClientMsg::OfferDraw,
                protocol::ClientMsg::AnswerDraw { accept } => ClientMsg::AnswerDraw { accept },
                protocol::ClientMsg::AskTakeback => ClientMsg::AskTakeback,
                protocol::ClientMsg::AnswerTakeback { accept } => {
                    ClientMsg::AnswerTakeback { accept }
                }
                protocol::ClientMsg::OfferRematch => ClientMsg::OfferRematch,
                protocol::ClientMsg::AnswerRematch { accept } => {
                    ClientMsg::AnswerRematch { accept }
                }
            }
        }
    }

    impl From<protocol::Role> for Role {
        fn from(role: protocol::Role) -> Role {
            match role {
                protocol::Role::Player => Role::Player,
            }
        }
    }

    impl From<protocol::ServerMsg> for ServerMsg {
        fn from(message: protocol::ServerMsg) -> ServerMsg {
            match message {
                protocol::ServerMsg::Welcome { you, snapshot } => ServerMsg::Welcome {
                    you: you.into(),
                    snapshot: snapshot.into(),
                },
                protocol::ServerMsg::Moved { ply, cell } => ServerMsg::Moved {
                    ply: Ply(ply.into()),
                    cell: Cell(cell),
                },
                protocol::ServerMsg::Rejected { reason } => ServerMsg::Rejected {
                    reason: reason.into(),
                },
                protocol::ServerMsg::Ended(end) => ServerMsg::Ended(end.into()),
                protocol::ServerMsg::Offer(offer) => ServerMsg::Offer(offer.into()),
                protocol::ServerMsg::OfferAnswered { kind, accepted } => ServerMsg::OfferAnswered {
                    kind: kind.into(),
                    accepted,
                },
                protocol::ServerMsg::TookBack { to_ply } => ServerMsg::TookBack {
                    to_ply: Ply(to_ply.into()),
                },
                protocol::ServerMsg::RematchRoom { room } => ServerMsg::RematchRoom { room },
            }
        }
    }

    impl From<protocol::Snapshot> for Snapshot {
        fn from(snapshot: protocol::Snapshot) -> Snapshot {
            Snapshot {
                record: snapshot.record.into(),
                players: snapshot.players.into(),
                takebacks: snapshot.takebacks,
                offer: snapshot.offer.map(Offer::from),
                draws: snapshot.draws.into(),
                end: snapshot.end.map(GameEnd::from),
                rematch: snapshot.rematch,
            }
        }
    }

    impl From<protocol::Offer> for Offer {
        fn from(offer: protocol::Offer) -> Offer {
            Offer {
                kind: offer.kind.into(),
                by: offer.by.into(),
            }
        }
    }

    impl From<protocol::OfferKind> for OfferKind {
        fn from(kind: protocol::OfferKind) -> OfferKind {
            match kind {
                protocol::OfferKind::Draw => OfferKind::Draw,
                protocol::OfferKind::Takeback => OfferKind::Takeback,
                protocol::OfferKind::Rematch => OfferKind::Rematch,
            }
        }
    }

    impl From<protocol::Draws> for Draws {
        fn from(draws: protocol::Draws) -> Draws {
            Draws {
                x: draws.x,
                o: draws.o,
            }
        }
    }

    impl From<protocol::GameEnd> for GameEnd {
        fn from(end: protocol::GameEnd) -> GameEnd {
            GameEnd {
                result: end.result.into(),
                termination: end.termination.into(),
            }
        }
    }

    impl From<protocol::GameResult> for GameResult {
        fn from(result: protocol::GameResult) -> GameResult {
            match result {
                protocol::GameResult::XWins => GameResult::XWins,
                protocol::GameResult::OWins => GameResult::OWins,
                protocol::GameResult::Draw => GameResult::Draw,
            }
        }
    }

    impl From<protocol::Termination> for Termination {
        fn from(termination: protocol::Termination) -> Termination {
            match termination {
                protocol::Termination::Line => Termination::Line,
                protocol::Termination::Count => Termination::Count,
                protocol::Termination::Decided => Termination::Decided,
                protocol::Termination::Resign => Termination::Resign,
                protocol::Termination::AgreedDraw => Termination::AgreedDraw,
            }
        }
    }

    impl From<protocol::Players> for Players {
        fn from(players: protocol::Players) -> Players {
            Players {
                x: players.x,
                o: players.o,
            }
        }
    }

    impl From<protocol::Rejection> for Rejection {
        fn from(reason: protocol::Rejection) -> Rejection {
            match reason {
                protocol::Rejection::IllegalMove => Rejection::IllegalMove,
                protocol::Rejection::OutOfOrder { want } => Rejection::OutOfOrder {
                    want: Ply(want.into()),
                },
                protocol::Rejection::NotAllowed => Rejection::NotAllowed,
            }
        }
    }

    impl From<protocol::NewRoom> for NewRoom {
        fn from(asked: protocol::NewRoom) -> NewRoom {
            NewRoom {
                rules: asked.rules,
                seat: asked.seat.into(),
                takebacks: asked.takebacks,
            }
        }
    }

    impl From<protocol::SeatChoice> for SeatChoice {
        fn from(seat: protocol::SeatChoice) -> SeatChoice {
            match seat {
                protocol::SeatChoice::X => SeatChoice::X,
                protocol::SeatChoice::O => SeatChoice::O,
                protocol::SeatChoice::Random => SeatChoice::Random,
            }
        }
    }

    impl From<protocol::CreatedRoom> for CreatedRoom {
        fn from(created: protocol::CreatedRoom) -> CreatedRoom {
            CreatedRoom { room: created.room }
        }
    }

    impl From<protocol::RoomCard> for RoomCard {
        fn from(card: protocol::RoomCard) -> RoomCard {
            RoomCard {
                host: card.host,
                variant: card.variant,
                rules: card.rules,
                players: card.players.into(),
                you: card.you.map(Player::from),
                takebacks: card.takebacks,
            }
        }
    }
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing
)]
mod tests {
    use super::*;

    #[test]
    fn the_close_codes_are_the_protocols() {
        for close in protocol::CloseCode::ALL {
            assert_eq!(crate::CloseCode::from(close) as u16, close.code());
        }
    }

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
            Err(Malformed::Parse(engine::ParseError::BoardPath(2))),
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
    fn records_carry_their_rules_version() {
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
        let read = engine::Record::try_from(future).unwrap();
        assert_eq!(read.rules_version, engine::RULES_VERSION + 1);
        assert_eq!(
            engine::Game::open(&read).err(),
            Some(engine::OpenError::RulesVersion(engine::RULES_VERSION + 1))
        );
    }

    mod wire {
        use std::collections::BTreeSet;

        use serde::Serialize;
        use serde::de::DeserializeOwned;
        use tsify::Tsify;

        use super::*;
        use crate::wire::{ClientMsg, CreatedRoom, NewRoom, RoomCard, ServerMsg};

        /// The protocol's JSON for `sample` reads into its mirror as the conversion says, and the
        /// mirror writes it back byte for byte, so the `.d.ts` declares what the server sends and
        /// reads what the page sends.
        fn holds<P, M>(sample: P) -> serde_json::Value
        where
            P: Serialize + Clone,
            M: From<P> + Serialize + DeserializeOwned + PartialEq + std::fmt::Debug,
        {
            let json = serde_json::to_string(&sample).unwrap();
            let mirror: M = serde_json::from_str(&json).unwrap();
            assert_eq!(mirror, M::from(sample), "{json}");
            assert_eq!(serde_json::to_string(&mirror).unwrap(), json);
            serde_json::from_str(&json).unwrap()
        }

        /// Every `type` a declaration lists, so each message has a sample.
        fn declared<T: Tsify>() -> BTreeSet<String> {
            T::DECL
                .split("type: \"")
                .skip(1)
                .map(|rest| rest.split('"').next().unwrap().to_owned())
                .collect()
        }

        fn every<P, M>(samples: Vec<P>) -> BTreeSet<String>
        where
            P: Serialize + Clone,
            M: From<P> + Serialize + DeserializeOwned + PartialEq + std::fmt::Debug,
        {
            samples
                .into_iter()
                .map(|sample| holds::<P, M>(sample)["type"].as_str().unwrap().to_owned())
                .collect()
        }

        fn players(x: Option<&str>, o: Option<&str>) -> protocol::Players {
            protocol::Players {
                x: x.map(str::to_owned),
                o: o.map(str::to_owned),
            }
        }

        #[test]
        fn the_messages_are_the_protocols() {
            let client = vec![
                protocol::ClientMsg::Hello {
                    protocol: protocol::PROTOCOL,
                    rules_version: engine::RULES_VERSION,
                    room: "6f1c0a9b3e2d4c5a7b8e".into(),
                    role: protocol::Role::Player,
                },
                protocol::ClientMsg::Move { ply: 7, cell: 40 },
                protocol::ClientMsg::Resign,
                protocol::ClientMsg::OfferDraw,
                protocol::ClientMsg::AnswerDraw { accept: true },
                protocol::ClientMsg::AskTakeback,
                protocol::ClientMsg::AnswerTakeback { accept: false },
                protocol::ClientMsg::OfferRematch,
                protocol::ClientMsg::AnswerRematch { accept: true },
            ];
            assert_eq!(every::<_, ClientMsg>(client), declared::<ClientMsg>());

            let mut game =
                engine::Game::new(engine::Preset::Deep.ruleset(), engine::Seed(0x5eed00));
            let cell = engine::Cell::from_index(engine::Depth::Three, 364).unwrap();
            game.play(game.state().ply(), cell).unwrap();
            let rejected = |reason| protocol::ServerMsg::Rejected { reason };
            let ended = |result, termination| {
                protocol::ServerMsg::Ended(protocol::GameEnd {
                    result,
                    termination,
                })
            };
            let offer = |kind, by| protocol::Offer { kind, by };
            let server = vec![
                protocol::ServerMsg::Welcome {
                    you: engine::Player::O,
                    snapshot: protocol::Snapshot {
                        record: game.record(),
                        players: players(Some("guest-4821"), None),
                        takebacks: false,
                        offer: None,
                        draws: protocol::Draws::default(),
                        end: None,
                        rematch: None,
                    },
                },
                protocol::ServerMsg::Welcome {
                    you: engine::Player::X,
                    snapshot: protocol::Snapshot {
                        record: game.record(),
                        players: players(Some("guest-4821"), Some("guest-1066")),
                        takebacks: true,
                        offer: Some(offer(protocol::OfferKind::Rematch, engine::Player::O)),
                        draws: protocol::Draws { x: 3, o: 1 },
                        end: Some(protocol::GameEnd {
                            result: protocol::GameResult::Draw,
                            termination: protocol::Termination::AgreedDraw,
                        }),
                        rematch: Some("0123456789abcdef0123".into()),
                    },
                },
                protocol::ServerMsg::Moved { ply: 3, cell: 728 },
                rejected(protocol::Rejection::IllegalMove),
                rejected(protocol::Rejection::OutOfOrder { want: 5 }),
                rejected(protocol::Rejection::NotAllowed),
                ended(protocol::GameResult::XWins, protocol::Termination::Line),
                ended(protocol::GameResult::OWins, protocol::Termination::Count),
                ended(protocol::GameResult::Draw, protocol::Termination::Decided),
                ended(protocol::GameResult::OWins, protocol::Termination::Resign),
                ended(
                    protocol::GameResult::Draw,
                    protocol::Termination::AgreedDraw,
                ),
                protocol::ServerMsg::Offer(offer(protocol::OfferKind::Draw, engine::Player::X)),
                protocol::ServerMsg::Offer(offer(protocol::OfferKind::Takeback, engine::Player::O)),
                protocol::ServerMsg::OfferAnswered {
                    kind: protocol::OfferKind::Takeback,
                    accepted: true,
                },
                protocol::ServerMsg::TookBack { to_ply: 4 },
                protocol::ServerMsg::RematchRoom {
                    room: "6f1c0a9b3e2d4c5a7b8e".into(),
                },
            ];
            assert_eq!(every::<_, ServerMsg>(server), declared::<ServerMsg>());
        }

        #[test]
        fn the_room_routes_are_the_protocols() {
            for seat in [
                protocol::SeatChoice::X,
                protocol::SeatChoice::O,
                protocol::SeatChoice::Random,
            ] {
                let asked = protocol::NewRoom {
                    rules: "cont+first5+most".into(),
                    seat,
                    takebacks: seat == protocol::SeatChoice::Random,
                };
                holds::<_, NewRoom>(asked);
            }
            holds::<_, CreatedRoom>(protocol::CreatedRoom {
                room: "6f1c0a9b3e2d4c5a7b8e".into(),
            });
            for you in [None, Some(engine::Player::X)] {
                holds::<_, RoomCard>(protocol::RoomCard {
                    host: "guest-4821".into(),
                    variant: "Standard".into(),
                    rules: "std".into(),
                    players: players(Some("guest-4821"), Some("guest-1066")),
                    you,
                    takebacks: you.is_some(),
                });
            }
        }
    }
}
