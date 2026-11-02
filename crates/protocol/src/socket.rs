//! The messages on a Room's socket (spec §10.3): JSON objects tagged by `type`. Clients send
//! intents; the server checks each against the engine and broadcasts what happened, stamped with
//! the ply. Cells cross as their base-9 index, read against the Room's depth.

use alloc::string::String;

use kokonoe_engine::{Fallback, Objective, Outcome, Player, Record, Ruleset};

use crate::{Abort, Clocks, TimeControl};

/// What a client says.
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(tag = "type")
)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ClientMsg {
    /// The first message on every socket: which build is talking, and which Room it wants. A build
    /// on another [`PROTOCOL`](crate::PROTOCOL) or `RULES_VERSION` is closed with 4001.
    Hello {
        protocol: u32,
        rules_version: u32,
        room: String,
        role: Role,
    },
    /// A move at `ply`, which counts the dealt opening. Idempotent by ply, so a resend after a
    /// reconnect changes nothing.
    Move { ply: u16, cell: u16 },
    /// Gives the game up: the other player wins.
    Resign,
    /// Offers a draw, at most [`Draws::MAX`] times a game. It stands until it's answered or the
    /// offerer moves.
    OfferDraw,
    /// Accepts or declines the other player's draw offer.
    AnswerDraw { accept: bool },
    /// Asks to undo back to before this player's last move, in a Room that allows takebacks. The
    /// other player answers before it lapses, and any move withdraws it.
    AskTakeback,
    /// Accepts or declines the other player's takeback.
    AnswerTakeback { accept: bool },
    /// Offers another game, X and O swapped, once this one is over and before it has had one. It
    /// lapses if it isn't answered.
    OfferRematch,
    /// Accepts or declines the other player's rematch.
    AnswerRematch { accept: bool },
    /// Asks the server's time, to set the page's clocks by: `t` is the page's own, in whole ms,
    /// which the `Pong` returns.
    Ping { t: u64 },
}

/// What a socket comes to a Room as. Spectators join it with KOK-61.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    Player,
}

/// What the server says.
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(tag = "type")
)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ServerMsg {
    /// The whole game as it stands, and the seat this socket plays. Sent after `Hello`, and again
    /// to every socket when a seat fills, so whoever was waiting learns who joined. A client
    /// replaces everything it knew with it.
    Welcome { you: Player, snapshot: Snapshot },
    /// A move was played and stored, and in a timed Room, the clocks after it.
    Moved {
        ply: u16,
        cell: u16,
        clocks: Option<Clocks>,
    },
    /// The sender's message changed nothing.
    Rejected { reason: Rejection },
    /// The game is over: after the move, resignation or agreed draw that ended it, or to a move
    /// sent after. It withdraws a draw offer or takeback still waiting. Rating changes join it
    /// with KOK-62.
    Ended(GameEnd),
    /// A player offered a draw, a takeback or a rematch, which the other answers.
    Offer(Offer),
    /// The offer stands no more: answered, lapsed, or withdrawn by a move.
    OfferAnswered { kind: OfferKind, accepted: bool },
    /// An accepted takeback: the game, and in a timed Room its clocks, as they were after
    /// `to_ply` plies.
    TookBack { to_ply: u16, clocks: Option<Clocks> },
    /// An accepted rematch: the new Room, where both players already have their seats.
    RematchRoom { room: String },
    /// The game ended without a result, and isn't kept: the Room is gone.
    Aborted { reason: Abort },
    /// The answer to a `Ping`: its `t`, and the server's Unix time in ms.
    Pong { t: u64, server_ms: u64 },
}

/// A Room as it stands.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Snapshot {
    /// Spec §4.3's Record, every move so far included.
    pub record: Record,
    pub players: Players,
    /// Whether the Room was made allowing takebacks.
    pub takebacks: bool,
    /// The offer waiting for an answer, if one is.
    pub offer: Option<Offer>,
    /// How many draws each player has offered, of the [`Draws::MAX`] a game allows.
    pub draws: Draws,
    /// How the game ended, once it has: a resignation or an agreed draw can't be read from the
    /// Record.
    pub end: Option<GameEnd>,
    /// The Room an accepted rematch made, so a player who missed `RematchRoom` can follow it.
    pub rematch: Option<String>,
    /// The time control, or None for an untimed Room.
    pub clock: Option<TimeControl>,
    /// The clocks now, in a timed Room.
    pub clocks: Option<Clocks>,
}

/// What a player offered, and how long it waits for its answer.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Offer {
    pub kind: OfferKind,
    pub by: Player,
    /// Milliseconds left before the offer lapses, as of this message. None for a draw offer,
    /// which stands until it's answered or its player moves.
    pub lapses_in: Option<u32>,
}

/// What an offer is for.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OfferKind {
    Draw,
    Takeback,
    Rematch,
}

/// How many draws each player has offered.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Draws {
    #[cfg_attr(feature = "serde", serde(rename = "X"))]
    pub x: u8,
    #[cfg_attr(feature = "serde", serde(rename = "O"))]
    pub o: u8,
}

impl Draws {
    /// How many draws a player may offer in one game.
    pub const MAX: u8 = 3;

    /// The draws `player` has offered.
    pub fn get(&self, player: Player) -> u8 {
        match player {
            Player::X => self.x,
            Player::O => self.o,
        }
    }

    /// Counts one more draw offered by `player`.
    pub fn bump(&mut self, player: Player) {
        let made = match player {
            Player::X => &mut self.x,
            Player::O => &mut self.o,
        };
        *made = made.saturating_add(1);
    }
}

/// How a game ended: spec §4.4's Result and Termination.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GameEnd {
    pub result: GameResult,
    pub termination: Termination,
}

impl GameEnd {
    /// The end the rules gave the game.
    pub fn of(outcome: Outcome, ruleset: &Ruleset) -> GameEnd {
        GameEnd {
            result: GameResult::won_by(outcome.winner()),
            termination: Termination::of(outcome, ruleset),
        }
    }
}

/// Spec §4.4's Result, as a game file and the `games` table write it.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GameResult {
    #[cfg_attr(feature = "serde", serde(rename = "1-0"))]
    XWins,
    #[cfg_attr(feature = "serde", serde(rename = "0-1"))]
    OWins,
    #[cfg_attr(feature = "serde", serde(rename = "1/2-1/2"))]
    Draw,
}

impl GameResult {
    pub const ALL: [GameResult; 3] = [GameResult::XWins, GameResult::OWins, GameResult::Draw];

    /// The Result of a game `winner` won, or a draw without one.
    pub const fn won_by(winner: Option<Player>) -> GameResult {
        match winner {
            Some(Player::X) => GameResult::XWins,
            Some(Player::O) => GameResult::OWins,
            None => GameResult::Draw,
        }
    }

    /// The Result as spec §4.4 spells it, on the wire and in the `games` table.
    pub const fn as_str(self) -> &'static str {
        match self {
            GameResult::XWins => "1-0",
            GameResult::OWins => "0-1",
            GameResult::Draw => "1/2-1/2",
        }
    }

    /// The Result `text` spells, if it spells one.
    pub fn parse(text: &str) -> Option<GameResult> {
        GameResult::ALL
            .into_iter()
            .find(|result| result.as_str() == text)
    }
}

/// Spec §4.4's Termination: how a game came to its end. Claims and abandonment arrive with
/// presence.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Termination {
    #[cfg_attr(feature = "serde", serde(rename = "line"))]
    Line,
    #[cfg_attr(feature = "serde", serde(rename = "count"))]
    Count,
    #[cfg_attr(feature = "serde", serde(rename = "decided"))]
    Decided,
    #[cfg_attr(feature = "serde", serde(rename = "resign"))]
    Resign,
    #[cfg_attr(feature = "serde", serde(rename = "agreed draw"))]
    AgreedDraw,
    #[cfg_attr(feature = "serde", serde(rename = "timeout"))]
    Timeout,
}

impl Termination {
    pub const ALL: [Termination; 6] = [
        Termination::Line,
        Termination::Count,
        Termination::Decided,
        Termination::Resign,
        Termination::AgreedDraw,
        Termination::Timeout,
    ];

    /// How the rules ended a game (spec §4.4's table).
    pub fn of(outcome: Outcome, ruleset: &Ruleset) -> Termination {
        match outcome {
            Outcome::Objective(_) if matches!(ruleset.objective, Objective::Line) => {
                Termination::Line
            }
            Outcome::Objective(_) | Outcome::MostWon(_) => Termination::Count,
            Outcome::Draw if matches!(ruleset.fallback, Fallback::MostWon) => Termination::Count,
            Outcome::Draw => Termination::Decided,
        }
    }

    /// The Termination as spec §4.4 spells it, on the wire and in the `games` table.
    pub const fn as_str(self) -> &'static str {
        match self {
            Termination::Line => "line",
            Termination::Count => "count",
            Termination::Decided => "decided",
            Termination::Resign => "resign",
            Termination::AgreedDraw => "agreed draw",
            Termination::Timeout => "timeout",
        }
    }

    /// The Termination `text` spells, if it spells one.
    pub fn parse(text: &str) -> Option<Termination> {
        Termination::ALL
            .into_iter()
            .find(|termination| termination.as_str() == text)
    }
}

/// Who holds each seat, by Handle: `null` while it waits for someone.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Players {
    #[cfg_attr(feature = "serde", serde(rename = "X"))]
    pub x: Option<String>,
    #[cfg_attr(feature = "serde", serde(rename = "O"))]
    pub o: Option<String>,
}

impl Players {
    pub fn get(&self, player: Player) -> Option<&str> {
        match player {
            Player::X => self.x.as_deref(),
            Player::O => self.o.as_deref(),
        }
    }
}

/// Why a message changed nothing.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rejection {
    /// The rules don't allow that Cell now, or there's no such Cell.
    IllegalMove,
    /// The ply isn't the next one, nor a move already made there; `want` is the next. The client
    /// is behind, and reconnects for a fresh `Welcome`.
    OutOfOrder { want: u16 },
    /// It isn't this socket's to do now: the other seat's turn, the other seat still empty, a
    /// finished game, an offer already waiting, or no such offer to answer.
    NotAllowed,
}

#[cfg(test)]
mod tests {
    use alloc::string::ToString;
    use alloc::vec;

    use kokonoe_engine::{Cell, Preset, RULES_VERSION, Seed};

    use super::*;

    fn pinned<T>(message: &T, json: &str)
    where
        T: serde::Serialize + serde::de::DeserializeOwned + PartialEq + core::fmt::Debug,
    {
        assert_eq!(serde_json::to_string(message).unwrap(), json);
        assert_eq!(&serde_json::from_str::<T>(json).unwrap(), message);
    }

    #[test]
    fn a_client_says_hello_then_moves() {
        let hello = ClientMsg::Hello {
            protocol: 1,
            rules_version: 1,
            room: "6f1c0a9b3e2d4c5a7b8e".to_string(),
            role: Role::Player,
        };
        pinned(
            &hello,
            r#"{"type":"Hello","protocol":1,"rules_version":1,"room":"6f1c0a9b3e2d4c5a7b8e","role":"Player"}"#,
        );
        pinned(
            &ClientMsg::Move { ply: 0, cell: 40 },
            r#"{"type":"Move","ply":0,"cell":40}"#,
        );
    }

    #[test]
    fn the_server_tags_what_happened() {
        let record = Record {
            rules_version: RULES_VERSION,
            ruleset: Preset::Standard.ruleset(),
            seed: Seed(0x5eed00),
            moves: vec![Cell::from_digits(&[4, 2]).unwrap()],
        };
        let clocks = Clocks {
            x_ms: 180_000,
            o_ms: 180_000,
            running: None,
            at: 1_793_600_000_000,
            aborts_at: Some(1_793_600_030_000),
        };
        let snapshot = Snapshot {
            record,
            players: Players {
                x: Some("guest-4821".to_string()),
                o: None,
            },
            takebacks: true,
            offer: Some(Offer {
                kind: OfferKind::Draw,
                by: Player::X,
                lapses_in: None,
            }),
            draws: Draws { x: 1, o: 0 },
            end: None,
            rematch: None,
            clock: Some(TimeControl::ThreePlusTwo),
            clocks: Some(clocks),
        };
        let untimed = Snapshot {
            clock: None,
            clocks: None,
            ..snapshot.clone()
        };
        let json = serde_json::to_value(&untimed).unwrap();
        assert_eq!(
            (&json["clock"], &json["clocks"]),
            (&serde_json::Value::Null, &serde_json::Value::Null),
            "an untimed Room's are null"
        );
        let welcome = ServerMsg::Welcome {
            you: Player::O,
            snapshot,
        };
        pinned(
            &welcome,
            concat!(
                r#"{"type":"Welcome","you":"O","snapshot":{"record":{"rules_version":1,"#,
                r#""ruleset":{"depth":2,"continuation":"Off","drawn_owner":"Neither","#,
                r#""objective":"Line","fallback":"Draw","opening":"Free"},"#,
                r#""seed":"00000000005eed00","moves":[38]},"#,
                r#""players":{"X":"guest-4821","O":null},"takebacks":true,"#,
                r#""offer":{"kind":"Draw","by":"X","lapses_in":null},"draws":{"X":1,"O":0},"end":null,"rematch":null,"#,
                r#""clock":"3+2","clocks":{"x_ms":180000,"o_ms":180000,"running":null,"#,
                r#""at":1793600000000,"aborts_at":1793600030000}}}"#
            ),
        );
        pinned(
            &ServerMsg::Moved {
                ply: 1,
                cell: 22,
                clocks: None,
            },
            r#"{"type":"Moved","ply":1,"cell":22,"clocks":null}"#,
        );
        let running = Clocks {
            x_ms: 171_250,
            running: Some(Player::X),
            aborts_at: None,
            ..clocks
        };
        pinned(
            &ServerMsg::Moved {
                ply: 2,
                cell: 40,
                clocks: Some(running),
            },
            concat!(
                r#"{"type":"Moved","ply":2,"cell":40,"clocks":{"x_ms":171250,"o_ms":180000,"#,
                r#""running":"X","at":1793600000000,"aborts_at":null}}"#
            ),
        );
        let reasons = [
            (Rejection::IllegalMove, r#""IllegalMove""#),
            (
                Rejection::OutOfOrder { want: 5 },
                r#"{"OutOfOrder":{"want":5}}"#,
            ),
            (Rejection::NotAllowed, r#""NotAllowed""#),
        ];
        for (reason, json) in reasons {
            pinned(
                &ServerMsg::Rejected { reason },
                &alloc::format!(r#"{{"type":"Rejected","reason":{json}}}"#),
            );
        }
        let ended = GameEnd {
            result: GameResult::OWins,
            termination: Termination::Resign,
        };
        pinned(
            &ServerMsg::Ended(ended),
            r#"{"type":"Ended","result":"0-1","termination":"resign"}"#,
        );
        pinned(
            &ServerMsg::Offer(Offer {
                kind: OfferKind::Takeback,
                by: Player::O,
                lapses_in: Some(15_000),
            }),
            r#"{"type":"Offer","kind":"Takeback","by":"O","lapses_in":15000}"#,
        );
        pinned(
            &ServerMsg::OfferAnswered {
                kind: OfferKind::Rematch,
                accepted: false,
            },
            r#"{"type":"OfferAnswered","kind":"Rematch","accepted":false}"#,
        );
        pinned(
            &ServerMsg::TookBack {
                to_ply: 4,
                clocks: None,
            },
            r#"{"type":"TookBack","to_ply":4,"clocks":null}"#,
        );
        let restored = Clocks {
            running: Some(Player::O),
            ..running
        };
        pinned(
            &ServerMsg::TookBack {
                to_ply: 3,
                clocks: Some(restored),
            },
            concat!(
                r#"{"type":"TookBack","to_ply":3,"clocks":{"x_ms":171250,"o_ms":180000,"#,
                r#""running":"O","at":1793600000000,"aborts_at":null}}"#
            ),
        );
        pinned(
            &ServerMsg::Aborted {
                reason: Abort::FirstMoveTimeout,
            },
            r#"{"type":"Aborted","reason":"FirstMoveTimeout"}"#,
        );
        pinned(
            &ServerMsg::Pong {
                t: 12_345,
                server_ms: 1_793_600_000_000,
            },
            r#"{"type":"Pong","t":12345,"server_ms":1793600000000}"#,
        );
        pinned(
            &ServerMsg::RematchRoom {
                room: "6f1c0a9b3e2d4c5a7b8e".to_string(),
            },
            r#"{"type":"RematchRoom","room":"6f1c0a9b3e2d4c5a7b8e"}"#,
        );
    }

    #[test]
    fn a_client_offers_and_answers() {
        let intents = [
            (ClientMsg::Resign, r#"{"type":"Resign"}"#),
            (ClientMsg::OfferDraw, r#"{"type":"OfferDraw"}"#),
            (
                ClientMsg::AnswerDraw { accept: true },
                r#"{"type":"AnswerDraw","accept":true}"#,
            ),
            (ClientMsg::AskTakeback, r#"{"type":"AskTakeback"}"#),
            (
                ClientMsg::AnswerTakeback { accept: false },
                r#"{"type":"AnswerTakeback","accept":false}"#,
            ),
            (ClientMsg::OfferRematch, r#"{"type":"OfferRematch"}"#),
            (
                ClientMsg::AnswerRematch { accept: true },
                r#"{"type":"AnswerRematch","accept":true}"#,
            ),
            (
                ClientMsg::Ping { t: 12_345 },
                r#"{"type":"Ping","t":12345}"#,
            ),
        ];
        for (intent, json) in intents {
            pinned(&intent, json);
        }
    }

    #[test]
    fn the_rules_end_name_their_termination() {
        let standard = Preset::Standard.ruleset();
        let counting = Preset::TicTacKu.ruleset();
        let most = Preset::CountTiebreak.ruleset();
        let cases = [
            (Outcome::Objective(Player::X), &standard, "1-0", "line"),
            (Outcome::MostWon(Player::O), &standard, "0-1", "count"),
            (Outcome::Draw, &standard, "1/2-1/2", "decided"),
            (Outcome::Objective(Player::O), &counting, "0-1", "count"),
            (Outcome::Draw, &most, "1/2-1/2", "count"),
        ];
        for (outcome, ruleset, result, termination) in cases {
            let end = GameEnd::of(outcome, ruleset);
            assert_eq!(end.result.as_str(), result, "{outcome:?}");
            assert_eq!(end.termination.as_str(), termination, "{outcome:?}");
            assert_eq!(GameResult::parse(result), Some(end.result));
            assert_eq!(Termination::parse(termination), Some(end.termination));
        }
        for termination in Termination::ALL {
            assert_eq!(
                serde_json::to_value(termination).unwrap(),
                termination.as_str(),
                "the wire and the table spell it alike"
            );
        }
        for result in GameResult::ALL {
            assert_eq!(
                serde_json::to_value(result).unwrap(),
                result.as_str(),
                "the wire and the table spell it alike"
            );
        }
    }

    #[test]
    fn an_unknown_message_is_refused() {
        for json in [
            r#"{"type":"Resigned"}"#,
            r#"{"ply":0,"cell":40}"#,
            r#"{"type":"Move","ply":-1,"cell":40}"#,
        ] {
            assert!(serde_json::from_str::<ClientMsg>(json).is_err(), "{json}");
        }
    }
}
