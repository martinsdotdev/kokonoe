//! A Room in play: one task owns its Game, its seats and its sockets, and takes their commands in
//! turn, so two moves can never race. It writes each move to Postgres before telling anyone, and
//! when a write fails it stops, closing every socket, rather than guess what was stored: a write
//! that timed out may still have landed. The next `Hello` loads the Room again from the table.

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
    /// A socket asking for its seat. It gets one, or `Full`.
    Join {
        account: AccountId,
        handle: String,
        out: mpsc::Sender<ServerMsg>,
        reply: oneshot::Sender<Result<Conn, Full>>,
    },
    Move {
        conn: Conn,
        ply: u16,
        cell: u16,
    },
}

/// Both seats are someone else's.
#[derive(Debug)]
pub struct Full;

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
                Command::Join {
                    account,
                    handle,
                    out,
                    reply,
                } => self.join(account, handle, out, reply).await,
                Command::Move { conn, ply, cell } => self.play(conn, ply, cell).await,
            };
            if done.is_err() {
                tracing::error!(room = %self.id, "a Room stopped: what's stored is unknown");
                return;
            }
        }
    }

    async fn join(
        &mut self,
        account: AccountId,
        handle: String,
        out: mpsc::Sender<ServerMsg>,
        reply: oneshot::Sender<Result<Conn, Full>>,
    ) -> Result<(), Broken> {
        let held = self.seats.of(account);
        let player = match held {
            Some(player) => player,
            None => {
                let free = [Player::X, Player::O]
                    .into_iter()
                    .find(|&player| self.seats.get(player).is_none());
                let Some(player) = free else {
                    let _ = reply.send(Err(Full));
                    return Ok(());
                };
                let seated = games::seat(&self.pool, &self.id, player, account);
                if !written(tokio::time::timeout(WRITE_TIMEOUT, seated).await) {
                    return Err(Broken);
                }
                self.seats.set(player, Seat { account, handle });
                player
            }
        };
        let conn = Conn(self.next);
        self.next += 1;
        self.sockets.insert(conn, Socket { player, out });
        let _ = reply.send(Ok(conn));
        if held.is_some() {
            self.welcome(conn);
        } else {
            let every: Vec<Conn> = self.sockets.keys().copied().collect();
            for conn in every {
                self.welcome(conn);
            }
        }
        Ok(())
    }

    async fn play(&mut self, conn: Conn, ply: u16, cell: u16) -> Result<(), Broken> {
        let Some(player) = self.sockets.get(&conn).map(|socket| socket.player) else {
            return Ok(());
        };
        let reject = |room: &mut Room, reason| room.send(conn, ServerMsg::Rejected { reason });
        if let Status::Over(outcome) = self.game.state().status() {
            self.send(conn, ServerMsg::Ended { outcome });
            return Ok(());
        }
        let seated = self.seats.x.is_some() && self.seats.o.is_some();
        if !seated || Ply::new(ply).player() != player {
            reject(self, Rejection::NotAllowed);
            return Ok(());
        }
        let depth = self.game.state().ruleset().depth;
        let Ok(at) = Cell::from_index(depth, cell) else {
            reject(self, Rejection::IllegalMove);
            return Ok(());
        };
        let status = match self.game.play(Ply::new(ply), at) {
            Ok(Played::Now(status)) => status,
            Ok(Played::Already) => return Ok(()),
            Err(PlayError::Illegal(_)) => {
                reject(self, Rejection::IllegalMove);
                return Ok(());
            }
            Err(PlayError::OutOfOrder { want, .. }) => {
                let want = want.get();
                reject(self, Rejection::OutOfOrder { want });
                return Ok(());
            }
        };
        let ended = match status {
            Status::Over(outcome) => Some(outcome),
            Status::ToMove { .. } => None,
        };
        let before = self.game.moves().len() - 1;
        let appended = games::append(&self.pool, &self.id, before, cell, ended);
        if !written(tokio::time::timeout(WRITE_TIMEOUT, appended).await) {
            return Err(Broken);
        }
        self.broadcast(ServerMsg::Moved { ply, cell });
        if let Some(outcome) = ended {
            self.broadcast(ServerMsg::Ended { outcome });
        }
        Ok(())
    }

    fn welcome(&mut self, conn: Conn) {
        let Some(you) = self.sockets.get(&conn).map(|socket| socket.player) else {
            return;
        };
        let snapshot = Snapshot {
            record: self.game.record(),
            players: self.seats.players(),
        };
        self.send(conn, ServerMsg::Welcome { you, snapshot });
    }

    fn broadcast(&mut self, message: ServerMsg) {
        let every: Vec<Conn> = self.sockets.keys().copied().collect();
        for conn in every {
            self.send(conn, message.clone());
        }
    }

    /// Queues `message` for one socket. A socket that's gone, or so far behind its queue is full,
    /// is dropped; dropping its queue closes it, and its page reconnects into a fresh `Welcome`.
    fn send(&mut self, conn: Conn, message: ServerMsg) {
        let sent = self
            .sockets
            .get(&conn)
            .map(|socket| socket.out.try_send(message));
        if let Some(Err(_)) = sent {
            self.sockets.remove(&conn);
        }
    }
}

/// Whether a write finished in time and changed the one row it meant to.
fn written(done: Result<Result<bool, sqlx::Error>, tokio::time::error::Elapsed>) -> bool {
    match done {
        Ok(Ok(changed)) => changed,
        Ok(Err(error)) => {
            tracing::error!(%error, "a Room's write failed");
            false
        }
        Err(_) => {
            tracing::error!("a Room's write took too long");
            false
        }
    }
}
