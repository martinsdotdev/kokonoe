//! The Room's messages and routes (spec §10.3) as Rust mirrors of their TypeScript shapes, for
//! the declarations alone: the page sends and reads them as JSON itself, so nothing here is
//! built or parsed in Rust. Their serde attributes are `kokonoe-protocol`'s, which tsify reads
//! for the `.d.ts`; `convert`'s tests hold each mirror to the JSON the server writes. A missing
//! value is JSON's `null`, which tsify would declare `undefined`, so those fields say so.

use tsify::Tsify;

use crate::types::{Cell, Outcome, Player, Ply, Record};

/// What a client says on a Room's socket.
#[derive(Tsify, Clone, Debug, PartialEq, Eq)]
#[cfg_attr(test, derive(serde::Serialize, serde::Deserialize))]
#[serde(tag = "type")]
pub enum ClientMsg {
    Hello {
        protocol: u32,
        rules_version: u32,
        room: String,
        role: Role,
    },
    Move {
        ply: Ply,
        cell: Cell,
    },
}

#[derive(Tsify, Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(test, derive(serde::Serialize, serde::Deserialize))]
pub enum Role {
    Player,
}

/// What the server says on a Room's socket. A `Welcome` comes after `Hello`, and again to every
/// socket when a seat fills; it replaces everything the page knew.
#[derive(Tsify, Clone, Debug, PartialEq, Eq)]
#[cfg_attr(test, derive(serde::Serialize, serde::Deserialize))]
#[serde(tag = "type")]
pub enum ServerMsg {
    Welcome { you: Player, snapshot: Snapshot },
    Moved { ply: Ply, cell: Cell },
    Rejected { reason: Rejection },
    Ended { outcome: Outcome },
}

#[derive(Tsify, Clone, Debug, PartialEq, Eq)]
#[cfg_attr(test, derive(serde::Serialize, serde::Deserialize))]
pub struct Snapshot {
    pub record: Record,
    pub players: Players,
}

/// Who holds each seat, by Handle: `null` while it waits for someone.
#[derive(Tsify, Clone, Debug, PartialEq, Eq)]
#[cfg_attr(test, derive(serde::Serialize, serde::Deserialize))]
pub struct Players {
    #[serde(rename = "X")]
    #[tsify(type = "string | null")]
    pub x: Option<String>,
    #[serde(rename = "O")]
    #[tsify(type = "string | null")]
    pub o: Option<String>,
}

/// Why the server refused a move. `OutOfOrder` means the page is behind: it reconnects.
#[derive(Tsify, Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(test, derive(serde::Serialize, serde::Deserialize))]
pub enum Rejection {
    IllegalMove,
    OutOfOrder { want: Ply },
    NotAllowed,
}

/// `POST /api/rooms`: a Preset by its Ruleset code, and the host's seat.
#[derive(Tsify, Clone, Debug, PartialEq, Eq)]
#[cfg_attr(test, derive(serde::Serialize, serde::Deserialize))]
pub struct NewRoom {
    pub rules: String,
    pub seat: SeatChoice,
}

#[derive(Tsify, Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(test, derive(serde::Serialize, serde::Deserialize))]
pub enum SeatChoice {
    X,
    O,
    Random,
}

/// What `POST /api/rooms` answers: the Room's link is `/r/<room>`.
#[derive(Tsify, Clone, Debug, PartialEq, Eq)]
#[cfg_attr(test, derive(serde::Serialize, serde::Deserialize))]
pub struct CreatedRoom {
    pub room: String,
}

/// What `GET /api/rooms/<room>` answers: the host's Handle, the Variant's name and Ruleset code,
/// who sits where, and the seat the asker already holds.
#[derive(Tsify, Clone, Debug, PartialEq, Eq)]
#[cfg_attr(test, derive(serde::Serialize, serde::Deserialize))]
pub struct RoomCard {
    pub host: String,
    pub variant: String,
    pub rules: String,
    pub players: Players,
    #[tsify(type = "Player | null")]
    pub you: Option<Player>,
}
