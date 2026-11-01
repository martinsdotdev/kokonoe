//! The messages on a Room's socket (spec §10.3): JSON objects tagged by `type`. Clients send
//! intents; the server checks each against the engine and broadcasts what happened, stamped with
//! the ply. Cells cross as their base-9 index, read against the Room's depth.

use alloc::string::String;

use kokonoe_engine::{Outcome, Player, Record};

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
    /// A move was played and stored.
    Moved { ply: u16, cell: u16 },
    /// The sender's move changed nothing.
    Rejected { reason: Rejection },
    /// The game is over: after the move that ended it, or to a move sent after.
    Ended { outcome: Outcome },
}

/// A Room as it stands.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Snapshot {
    /// Spec §4.3's Record, every move so far included.
    pub record: Record,
    pub players: Players,
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

/// Why a move changed nothing.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rejection {
    /// The rules don't allow that Cell now, or there's no such Cell.
    IllegalMove,
    /// The ply isn't the next one, nor a move already made there; `want` is the next. The client
    /// is behind, and reconnects for a fresh `Welcome`.
    OutOfOrder { want: u16 },
    /// It isn't this socket's move to make: the other seat's turn, or the other seat still empty.
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
        let welcome = ServerMsg::Welcome {
            you: Player::O,
            snapshot: Snapshot {
                record,
                players: Players {
                    x: Some("guest-4821".to_string()),
                    o: None,
                },
            },
        };
        pinned(
            &welcome,
            concat!(
                r#"{"type":"Welcome","you":"O","snapshot":{"record":{"rules_version":1,"#,
                r#""ruleset":{"depth":2,"continuation":"Off","drawn_owner":"Neither","#,
                r#""objective":"Line","fallback":"Draw","opening":"Free"},"#,
                r#""seed":"00000000005eed00","moves":[38]},"#,
                r#""players":{"X":"guest-4821","O":null}}}"#
            ),
        );
        pinned(
            &ServerMsg::Moved { ply: 1, cell: 22 },
            r#"{"type":"Moved","ply":1,"cell":22}"#,
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
        pinned(
            &ServerMsg::Ended {
                outcome: Outcome::Objective(Player::X),
            },
            r#"{"type":"Ended","outcome":{"Objective":"X"}}"#,
        );
    }

    #[test]
    fn an_unknown_message_is_refused() {
        for json in [
            r#"{"type":"Resign"}"#,
            r#"{"ply":0,"cell":40}"#,
            r#"{"type":"Move","ply":-1,"cell":40}"#,
        ] {
            assert!(serde_json::from_str::<ClientMsg>(json).is_err(), "{json}");
        }
    }
}
