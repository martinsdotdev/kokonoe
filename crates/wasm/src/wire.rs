//! The Room's messages and routes (spec §10.3) as Rust mirrors of their TypeScript shapes, for
//! the declarations alone: the page sends and reads them as JSON itself, so nothing here is
//! built or parsed in Rust. Their serde attributes are `kokonoe-protocol`'s, which tsify reads
//! for the `.d.ts`; `convert`'s tests hold each mirror to the JSON the server writes. A missing
//! value is JSON's `null`, which tsify would declare `undefined`, so those fields say so.

use tsify::Tsify;

use crate::types::{Cell, Player, Ply, Record};

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
    Resign,
    OfferDraw,
    AnswerDraw {
        accept: bool,
    },
    AskTakeback,
    AnswerTakeback {
        accept: bool,
    },
    OfferRematch,
    AnswerRematch {
        accept: bool,
    },
    Ping {
        t: u64,
    },
    Claim {
        #[serde(rename = "as")]
        kind: ClaimKind,
    },
}

/// What a player claims of a game the other has left.
#[derive(Tsify, Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(test, derive(serde::Serialize, serde::Deserialize))]
pub enum ClaimKind {
    Win,
    Draw,
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
    Welcome {
        you: Player,
        /// Boxed, being far larger than every other message.
        snapshot: Box<Snapshot>,
    },
    Moved {
        ply: Ply,
        cell: Cell,
        #[tsify(type = "Clocks | null")]
        clocks: Option<Clocks>,
    },
    Rejected {
        reason: Rejection,
    },
    Ended(GameEnd),
    Offer(Offer),
    OfferAnswered {
        kind: OfferKind,
        accepted: bool,
    },
    TookBack {
        to_ply: Ply,
        #[tsify(type = "Clocks | null")]
        clocks: Option<Clocks>,
    },
    RematchRoom {
        room: String,
    },
    Aborted {
        reason: Abort,
    },
    Pong {
        t: u64,
        server_ms: u64,
    },
    Presence {
        player: Player,
        #[tsify(type = "Away | null")]
        away: Option<Away>,
    },
}

/// A Room as it stands. `end` says how a finished game ended, which a Record can't for a
/// resignation; `offer` is the one waiting for an answer; `rematch` is the Room an accepted
/// rematch made; `clock` and `clocks` are a timed Room's; `presence` says who is away.
#[derive(Tsify, Clone, Debug, PartialEq, Eq)]
#[cfg_attr(test, derive(serde::Serialize, serde::Deserialize))]
pub struct Snapshot {
    pub record: Record,
    pub players: Players,
    pub takebacks: bool,
    #[tsify(type = "Offer | null")]
    pub offer: Option<Offer>,
    pub draws: Draws,
    #[tsify(type = "GameEnd | null")]
    pub end: Option<GameEnd>,
    #[tsify(type = "string | null")]
    pub rematch: Option<String>,
    #[tsify(type = "TimeControl | null")]
    pub clock: Option<TimeControl>,
    #[tsify(type = "Clocks | null")]
    pub clocks: Option<Clocks>,
    pub presence: Presence,
}

/// A player with no socket in the Room: since when, in the server's Unix ms, and the ms left, as
/// of the message, before the other player may Claim the game.
#[derive(Tsify, Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(test, derive(serde::Serialize, serde::Deserialize))]
pub struct Away {
    pub since: u64,
    pub claims_in: u32,
}

/// Each seat's player while they're away: `null` while they're in the Room, and for an empty
/// seat.
#[derive(Tsify, Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(test, derive(serde::Serialize, serde::Deserialize))]
pub struct Presence {
    #[serde(rename = "X")]
    #[tsify(type = "Away | null")]
    pub x: Option<Away>,
    #[serde(rename = "O")]
    #[tsify(type = "Away | null")]
    pub o: Option<Away>,
}

/// A Fischer time control, spelled as a game file's `TimeControl` tag: a bank in minutes, plus
/// an increment in seconds.
#[derive(Tsify, Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(test, derive(serde::Serialize, serde::Deserialize))]
pub enum TimeControl {
    #[serde(rename = "3+2")]
    ThreePlusTwo,
    #[serde(rename = "5+3")]
    FivePlusThree,
    #[serde(rename = "10+5")]
    TenPlusFive,
    #[serde(rename = "15+5")]
    FifteenPlusFive,
}

/// A timed game's clocks as they stood at `at`, the server's Unix ms: each player's time left,
/// whose runs, and when the game aborts unless the player to move makes their first move.
#[derive(Tsify, Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(test, derive(serde::Serialize, serde::Deserialize))]
pub struct Clocks {
    pub x_ms: u32,
    pub o_ms: u32,
    #[tsify(type = "Player | null")]
    pub running: Option<Player>,
    pub at: u64,
    #[tsify(type = "number | null")]
    pub aborts_at: Option<u64>,
}

/// Why a game ended without a result.
#[derive(Tsify, Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(test, derive(serde::Serialize, serde::Deserialize))]
pub enum Abort {
    FirstMoveTimeout,
}

#[derive(Tsify, Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(test, derive(serde::Serialize, serde::Deserialize))]
pub struct Offer {
    pub kind: OfferKind,
    pub by: Player,
    #[tsify(type = "number | null")]
    pub lapses_in: Option<u32>,
}

#[derive(Tsify, Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(test, derive(serde::Serialize, serde::Deserialize))]
pub enum OfferKind {
    Draw,
    Takeback,
    Rematch,
}

/// How many draws each player has offered, of the 3 a game allows.
#[derive(Tsify, Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(test, derive(serde::Serialize, serde::Deserialize))]
pub struct Draws {
    #[serde(rename = "X")]
    pub x: u8,
    #[serde(rename = "O")]
    pub o: u8,
}

/// Spec §4.4's Result and Termination. An Abandoned game has no result.
#[derive(Tsify, Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(test, derive(serde::Serialize, serde::Deserialize))]
pub struct GameEnd {
    #[tsify(type = "GameResult | null")]
    pub result: Option<GameResult>,
    pub termination: Termination,
}

#[derive(Tsify, Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(test, derive(serde::Serialize, serde::Deserialize))]
pub enum GameResult {
    #[serde(rename = "1-0")]
    XWins,
    #[serde(rename = "0-1")]
    OWins,
    #[serde(rename = "1/2-1/2")]
    Draw,
}

#[derive(Tsify, Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(test, derive(serde::Serialize, serde::Deserialize))]
pub enum Termination {
    #[serde(rename = "line")]
    Line,
    #[serde(rename = "count")]
    Count,
    #[serde(rename = "decided")]
    Decided,
    #[serde(rename = "resign")]
    Resign,
    #[serde(rename = "agreed draw")]
    AgreedDraw,
    #[serde(rename = "timeout")]
    Timeout,
    #[serde(rename = "claim")]
    Claim,
    #[serde(rename = "abandoned")]
    Abandoned,
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

/// `POST /api/rooms`: a Preset by its Ruleset code, the host's seat, whether takebacks are
/// allowed, and the time control, `null` for untimed.
#[derive(Tsify, Clone, Debug, PartialEq, Eq)]
#[cfg_attr(test, derive(serde::Serialize, serde::Deserialize))]
pub struct NewRoom {
    pub rules: String,
    pub seat: SeatChoice,
    pub takebacks: bool,
    #[tsify(type = "TimeControl | null")]
    pub clock: Option<TimeControl>,
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
    pub takebacks: bool,
    #[tsify(type = "TimeControl | null")]
    pub clock: Option<TimeControl>,
}
