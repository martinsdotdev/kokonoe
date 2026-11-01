//! What the server and its clients say to each other (spec §10.3), shared by the server and,
//! through `kokonoe-wasm`'s mirrors, the SPA: the Room routes' JSON, the messages on a Room's
//! socket, and the codes the server closes it with.

#![no_std]

extern crate alloc;

mod api;
mod close;
mod socket;

pub use api::{CreatedRoom, NewRoom, RoomCard, SeatChoice};
pub use close::CloseCode;
pub use socket::{ClientMsg, Players, Rejection, Role, ServerMsg, Snapshot};

/// The version of the socket's messages, sent in every `Hello`. Bumped whenever a message changes
/// shape, so a client on another version is closed with 4001 and reloads rather than misreading.
pub const PROTOCOL: u32 = 1;
