//! A Room in play: one task owns its Game, its seats and its sockets, and takes their commands in
//! turn, so two moves can never race. It writes each move to Postgres before telling anyone, and
//! when a write fails it stops, closing every socket, rather than guess what was stored: a write
//! that timed out may still have landed. It also stops once its last socket leaves. Either way the
//! next `Hello` loads the Room again from the table.

use std::collections::HashMap;
use std::fmt;
use std::time::Duration;

use kokonoe_engine::{Cell, Game, PlayError, Played, Player, Ply, Status};
use kokonoe_protocol::{Rejection, ServerMsg, Snapshot};
use sqlx::PgPool;
use tokio::sync::{mpsc, oneshot};

use crate::games::{self, Seat, Seats};
use crate::random::{bytes, hex};
use crate::session::AccountId;

/// How long a write may take before the Room gives up on it.
const WRITE_TIMEOUT: Duration = Duration::from_secs(2);

/// How many commands wait for a Room before its sockets wait too.
const COMMANDS: usize = 64;

/// A Room's id: 80 random bits as 20 lowercase hex digits, the `<room>` in `/r/<room>`.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct RoomId(String);

impl RoomId {
    pub fn new() -> RoomId {
        RoomId(hex(&bytes::<10>()))
    }

    /// The id `text` spells, or nothing if it can't be one, so a mistyped link costs no query.
    pub fn parse(text: &str) -> Option<RoomId> {
        let digits = text.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'));
        (text.len() == 20 && digits).then(|| RoomId(text.to_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for RoomId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// One socket in a Room.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Conn(u64);

pub enum Command {
    Join(Join),
    Move { conn: Conn, ply: u16, cell: u16 },
    Leave { conn: Conn },
}

/// A socket asking for a seat: the one its Account holds, or an empty one.
pub struct Join {
    pub account: AccountId,
    pub handle: String,
    /// Where the Room's messages for this socket go.
    pub out: mpsc::Sender<ServerMsg>,
    /// The socket's place, or nothing when both seats are someone else's.
    pub reply: oneshot::Sender<Option<Conn>>,
}

/// A write didn't do what the Room expected, so it stops.
struct Broken;

struct Room {
    id: RoomId,
    pool: PgPool,
    game: Game,
    seats: Seats,
    sockets: HashMap<Conn, Socket>,
    next: u64,
}

struct Socket {
    player: Player,
    out: mpsc::Sender<ServerMsg>,
}

/// Starts the Room `id` from what was stored, and gives the way to reach it.
pub fn spawn(id: RoomId, pool: PgPool, game: Game, seats: Seats) -> mpsc::Sender<Command> {
    let (commands, inbox) = mpsc::channel(COMMANDS);
    let room = Room {
        id,
        pool,
        game,
        seats,
        sockets: HashMap::new(),
        next: 0,
    };
    tokio::spawn(room.run(inbox));
    commands
}

impl Room {
    async fn run(mut self, mut inbox: mpsc::Receiver<Command>) {
        while let Some(command) = inbox.recv().await {
            let done = match command {
                Command::Join(join) => self.join(join).await,
                Command::Move { conn, ply, cell } => self.play(conn, ply, cell).await,
                Command::Leave { conn } => {
                    self.sockets.remove(&conn);
                    Ok(())
                }
            };
            if done.is_err() {
                tracing::error!(room = %self.id, "a Room stopped: what's stored is unknown");
                return;
            }
            if self.sockets.is_empty() {
                return;
            }
        }
    }

    async fn join(&mut self, join: Join) -> Result<(), Broken> {
        let Join {
            account,
            handle,
            out,
            reply,
        } = join;
        let (player, seated) = match self.seats.of(account) {
            Some(player) => (player, false),
            None => {
                let Some(player) = self.seats.empty() else {
                    let _ = reply.send(None);
                    return Ok(());
                };
                write(games::seat(&self.pool, &self.id, player, account)).await?;
                self.seats.set(player, Seat { account, handle });
                (player, true)
            }
        };
        let conn = Conn(self.next);
        self.next += 1;
        self.sockets.insert(conn, Socket { player, out });
        let _ = reply.send(Some(conn));
        // A seat that fills tells everyone, so whoever was waiting learns who joined.
        let snapshot = Snapshot {
            record: self.game.record(),
            players: self.seats.players(),
        };
        self.tell(|other, socket| {
            (seated || other == conn).then(|| ServerMsg::Welcome {
                you: socket.player,
                snapshot: snapshot.clone(),
            })
        });
        Ok(())
    }

    async fn play(&mut self, conn: Conn, ply: u16, cell: u16) -> Result<(), Broken> {
        let Some(player) = self.sockets.get(&conn).map(|socket| socket.player) else {
            return Ok(());
        };
        let status = match self.judge(player, ply, cell) {
            Ok(Some(status)) => status,
            Ok(None) => return Ok(()),
            Err(answer) => {
                self.send(conn, answer);
                return Ok(());
            }
        };
        let ended = match status {
            Status::Over(outcome) => Some(outcome),
            Status::ToMove { .. } => None,
        };
        let before = self.game.moves().len() - 1;
        write(games::append(&self.pool, &self.id, before, cell, ended)).await?;
        self.tell(|_, _| Some(ServerMsg::Moved { ply, cell }));
        if let Some(outcome) = ended {
            self.tell(|_, _| Some(ServerMsg::Ended { outcome }));
        }
        Ok(())
    }

    /// Plays `player`'s move at `ply` if it's theirs to make and the rules allow it, giving the
    /// game's status after it; nothing for a move already made; or the answer that refuses it.
    fn judge(&mut self, player: Player, ply: u16, cell: u16) -> Result<Option<Status>, ServerMsg> {
        let refuse = |reason| ServerMsg::Rejected { reason };
        if let Status::Over(outcome) = self.game.state().status() {
            return Err(ServerMsg::Ended { outcome });
        }
        if !self.seats.full() || Ply::new(ply).player() != player {
            return Err(refuse(Rejection::NotAllowed));
        }
        let depth = self.game.state().ruleset().depth;
        let cell = Cell::from_index(depth, cell).map_err(|_| refuse(Rejection::IllegalMove))?;
        match self.game.play(Ply::new(ply), cell) {
            Ok(Played::Now(status)) => Ok(Some(status)),
            Ok(Played::Already) => Ok(None),
            Err(PlayError::Illegal(_)) => Err(refuse(Rejection::IllegalMove)),
            Err(PlayError::OutOfOrder { want, .. }) => {
                Err(refuse(Rejection::OutOfOrder { want: want.get() }))
            }
        }
    }

    /// Queues each socket the message `to` gives it, if any. A socket that's gone, or so far
    /// behind that its queue is full, is dropped: dropping its queue closes it, and its page
    /// reconnects for a fresh `Welcome`.
    fn tell(&mut self, to: impl Fn(Conn, &Socket) -> Option<ServerMsg>) {
        self.sockets.retain(|&conn, socket| match to(conn, socket) {
            Some(message) => socket.out.try_send(message).is_ok(),
            None => true,
        });
    }

    fn send(&mut self, conn: Conn, message: ServerMsg) {
        if let Some(socket) = self.sockets.get(&conn)
            && socket.out.try_send(message).is_err()
        {
            self.sockets.remove(&conn);
        }
    }
}

/// Runs a write that must change the one row it means to, in time.
async fn write(write: impl Future<Output = Result<bool, sqlx::Error>>) -> Result<(), Broken> {
    match tokio::time::timeout(WRITE_TIMEOUT, write).await {
        Ok(Ok(true)) => return Ok(()),
        Ok(Ok(false)) => tracing::error!("a Room's write found the row other than it expected"),
        Ok(Err(error)) => tracing::error!(%error, "a Room's write failed"),
        Err(_) => tracing::error!("a Room's write took too long"),
    }
    Err(Broken)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_room_id_is_20_lowercase_hex_digits() {
        let id = RoomId::new();
        assert_eq!(RoomId::parse(id.as_str()), Some(id));
        assert!(RoomId::parse("6f1c0a9b3e2d4c5a7b8e").is_some());
        for wrong in [
            "",
            "abc",
            "6F1C0A9B3E2D4C5A7B8E",
            "6f1c0a9b3e2d4c5a7b8",
            "6f1c0a9b3e2d4c5a7b8e0",
            "6f1c0a9b3e2d4c5a7b8g",
            "../../../etc/passwd!",
        ] {
            assert_eq!(RoomId::parse(wrong), None, "{wrong}");
        }
    }
}
