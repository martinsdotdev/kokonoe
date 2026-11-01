//! A Room in play: one task owns its Game, its seats and its sockets, and takes their commands in
//! turn, so two moves can never race. It writes each move, ending and takeback to Postgres before
//! telling anyone, and when a write fails it stops, closing every socket, rather than guess what
//! was stored: a write that timed out may still have landed. It also stops once its last socket
//! leaves. Either way the next `Hello` loads the Room again from the table.
//!
//! The Room's policies (spec §10.2) live here too: resigning, draw offers, takebacks and rematch.
//! At most one offer waits for an answer at a time, in this task alone, until its deadline; a Room
//! that stops forgets it.

use std::collections::HashMap;
use std::fmt;
use std::time::Duration;

use kokonoe_engine::{Cell, Game, PlayError, Played, Player, Ply, Seed, Status};
use kokonoe_protocol::{
    ClientMsg, Draws, GameEnd, GameResult, Offer, OfferKind, Rejection, ServerMsg, Snapshot,
    Termination,
};
use sqlx::PgPool;
use tokio::sync::{mpsc, oneshot};
use tokio::time::Instant;

use crate::games::{self, Seat, Seats, Stored};
use crate::random::{bytes, hex};
use crate::session::AccountId;

/// How long a write may take before the Room gives up on it.
const WRITE_TIMEOUT: Duration = Duration::from_secs(2);

/// How many commands wait for a Room before its sockets wait too.
const COMMANDS: usize = 64;

/// How long an offer waits for its answer (spec §10.2). A draw offer has no deadline: it stands
/// until it's answered or the offerer moves.
#[derive(Clone, Copy, Debug)]
pub struct Timing {
    /// The other player answers a takeback within this, while the one who asked waits.
    pub takeback: Duration,
    /// A rematch lapses after this.
    pub rematch: Duration,
}

impl Default for Timing {
    fn default() -> Timing {
        Timing {
            takeback: Duration::from_secs(15),
            rematch: Duration::from_secs(60),
        }
    }
}

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
    Act { conn: Conn, act: Act },
    Leave { conn: Conn },
}

/// What a seated socket asks of the Room.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Act {
    Move { ply: u16, cell: u16 },
    Resign,
    OfferDraw,
    AnswerDraw(bool),
    AskTakeback,
    AnswerTakeback(bool),
    OfferRematch,
    AnswerRematch(bool),
}

impl Act {
    /// What `message` asks, or nothing for a `Hello`, which only opens a socket.
    pub fn of(message: ClientMsg) -> Option<Act> {
        Some(match message {
            ClientMsg::Hello { .. } => return None,
            ClientMsg::Move { ply, cell } => Act::Move { ply, cell },
            ClientMsg::Resign => Act::Resign,
            ClientMsg::OfferDraw => Act::OfferDraw,
            ClientMsg::AnswerDraw { accept } => Act::AnswerDraw(accept),
            ClientMsg::AskTakeback => Act::AskTakeback,
            ClientMsg::AnswerTakeback { accept } => Act::AnswerTakeback(accept),
            ClientMsg::OfferRematch => Act::OfferRematch,
            ClientMsg::AnswerRematch { accept } => Act::AnswerRematch(accept),
        })
    }
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
    timing: Timing,
    game: Game,
    seats: Seats,
    takebacks: bool,
    draws: Draws,
    /// How the game ended other than by its rules: a resignation or an agreed draw.
    stopped: Option<GameEnd>,
    offer: Option<Pending>,
    sockets: HashMap<Conn, Socket>,
    next: u64,
}

/// The offer waiting for an answer, and when it lapses.
struct Pending {
    offer: Offer,
    lapses: Option<Instant>,
}

struct Socket {
    player: Player,
    out: mpsc::Sender<ServerMsg>,
}

/// Starts the Room `id` from what was stored, its game opened from it, and gives the way to reach
/// it.
pub fn spawn(
    id: RoomId,
    pool: PgPool,
    timing: Timing,
    game: Game,
    stored: Stored,
) -> mpsc::Sender<Command> {
    let (commands, inbox) = mpsc::channel(COMMANDS);
    // A rules ending is the engine's to tell again; only these two need the table.
    let stopped = stored.end.filter(|end| {
        matches!(
            end.termination,
            Termination::Resign | Termination::AgreedDraw
        )
    });
    let room = Room {
        id,
        pool,
        timing,
        game,
        seats: stored.seats,
        takebacks: stored.takebacks,
        draws: stored.draws,
        stopped,
        offer: None,
        sockets: HashMap::new(),
        next: 0,
    };
    tokio::spawn(room.run(inbox));
    commands
}

impl Room {
    async fn run(mut self, mut inbox: mpsc::Receiver<Command>) {
        loop {
            let lapses = self.offer.as_ref().and_then(|pending| pending.lapses);
            let done = tokio::select! {
                command = inbox.recv() => match command {
                    Some(Command::Join(join)) => self.join(join).await,
                    Some(Command::Act { conn, act }) => self.act(conn, act).await,
                    Some(Command::Leave { conn }) => {
                        self.sockets.remove(&conn);
                        Ok(())
                    }
                    None => return,
                },
                () = until(lapses) => {
                    self.withdraw();
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
        let snapshot = self.snapshot();
        self.tell(|other, socket| {
            (seated || other == conn).then(|| ServerMsg::Welcome {
                you: socket.player,
                snapshot: snapshot.clone(),
            })
        });
        Ok(())
    }

    /// Does what a seated socket asked, or tells it why not.
    async fn act(&mut self, conn: Conn, act: Act) -> Result<(), Broken> {
        let Some(player) = self.sockets.get(&conn).map(|socket| socket.player) else {
            return Ok(());
        };
        let allowed = match act {
            Act::Move { ply, cell } => return self.play(conn, player, ply, cell).await,
            Act::Resign => self.resign(player).await?,
            Act::OfferDraw => self.offer_draw(player).await?,
            Act::AnswerDraw(accept) => self.answer(player, OfferKind::Draw, accept).await?,
            Act::AskTakeback => self.ask_takeback(player),
            Act::AnswerTakeback(accept) => self.answer(player, OfferKind::Takeback, accept).await?,
            Act::OfferRematch => self.offer_rematch(player),
            Act::AnswerRematch(accept) => self.answer(player, OfferKind::Rematch, accept).await?,
        };
        if !allowed {
            let reason = Rejection::NotAllowed;
            self.send(conn, ServerMsg::Rejected { reason });
        }
        Ok(())
    }

    async fn play(
        &mut self,
        conn: Conn,
        player: Player,
        ply: u16,
        cell: u16,
    ) -> Result<(), Broken> {
        let status = match self.judge(player, ply, cell) {
            Ok(Some(status)) => status,
            Ok(None) => return Ok(()),
            Err(answer) => {
                self.send(conn, answer);
                return Ok(());
            }
        };
        let ruleset = self.game.state().ruleset();
        let ended = match status {
            Status::Over(outcome) => Some(GameEnd::of(outcome, &ruleset)),
            Status::ToMove { .. } => None,
        };
        let before = self.game.moves().len() - 1;
        write(games::append(&self.pool, &self.id, before, cell, ended)).await?;
        self.tell(|_, _| Some(ServerMsg::Moved { ply, cell }));
        match ended {
            Some(end) => {
                self.offer = None;
                self.tell(|_, _| Some(ServerMsg::Ended(end)));
            }
            // A move withdraws a takeback, which was asked of the game before it, and the mover's
            // own draw offer.
            None => {
                let withdrawn = self.offer.as_ref().is_some_and(|pending| {
                    let Offer { kind, by } = pending.offer;
                    kind == OfferKind::Takeback || (kind == OfferKind::Draw && by == player)
                });
                if withdrawn {
                    self.withdraw();
                }
            }
        }
        Ok(())
    }

    /// Plays `player`'s move at `ply` if it's theirs to make and the rules allow it, giving the
    /// game's status after it; nothing for a move already made; or the answer that refuses it.
    fn judge(&mut self, player: Player, ply: u16, cell: u16) -> Result<Option<Status>, ServerMsg> {
        let refuse = |reason| ServerMsg::Rejected { reason };
        if let Some(end) = self.end() {
            return Err(ServerMsg::Ended(end));
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

    /// Whether the game goes on with both seats filled, so it can be resigned or drawn.
    fn playing(&self) -> bool {
        self.seats.full() && self.end().is_none()
    }

    async fn resign(&mut self, player: Player) -> Result<bool, Broken> {
        if !self.playing() {
            return Ok(false);
        }
        self.finish(GameEnd {
            result: GameResult::won_by(Some(player.opponent())),
            termination: Termination::Resign,
        })
        .await?;
        Ok(true)
    }

    async fn offer_draw(&mut self, player: Player) -> Result<bool, Broken> {
        let made = self.draws.get(player);
        if !self.playing() || self.offer.is_some() || made >= Draws::MAX {
            return Ok(false);
        }
        write(games::offer_draw(&self.pool, &self.id, player, made)).await?;
        match player {
            Player::X => self.draws.x += 1,
            Player::O => self.draws.o += 1,
        }
        self.propose(OfferKind::Draw, player, None);
        Ok(true)
    }

    fn ask_takeback(&mut self, player: Player) -> bool {
        let allowed = self.takebacks && self.playing() && self.offer.is_none();
        if !allowed || self.taken_back(player).is_none() {
            return false;
        }
        let lapses = Instant::now() + self.timing.takeback;
        self.propose(OfferKind::Takeback, player, Some(lapses));
        true
    }

    fn offer_rematch(&mut self, player: Player) -> bool {
        if !self.seats.full() || self.end().is_none() || self.offer.is_some() {
            return false;
        }
        let lapses = Instant::now() + self.timing.rematch;
        self.propose(OfferKind::Rematch, player, Some(lapses));
        true
    }

    /// `player`'s answer to the other's offer of `kind`, which must be the one waiting.
    async fn answer(
        &mut self,
        player: Player,
        kind: OfferKind,
        accept: bool,
    ) -> Result<bool, Broken> {
        let waiting = self
            .offer
            .as_ref()
            .map(|pending| pending.offer)
            .filter(|offer| offer.kind == kind && offer.by != player);
        let Some(offer) = waiting else {
            return Ok(false);
        };
        self.offer = None;
        if !accept {
            self.tell(|_, _| {
                Some(ServerMsg::OfferAnswered {
                    kind,
                    accepted: false,
                })
            });
            return Ok(true);
        }
        let accepted = ServerMsg::OfferAnswered {
            kind,
            accepted: true,
        };
        match kind {
            OfferKind::Draw => {
                self.tell(|_, _| Some(accepted.clone()));
                self.finish(GameEnd {
                    result: GameResult::Draw,
                    termination: Termination::AgreedDraw,
                })
                .await?;
            }
            OfferKind::Takeback => {
                // Nothing moved since it was asked, or the move would have withdrawn it.
                let Some(keep) = self.taken_back(offer.by) else {
                    return Ok(false);
                };
                let before = self.game.moves().len();
                write(games::take_back(&self.pool, &self.id, before, keep)).await?;
                for _ in keep..before {
                    self.game.undo();
                }
                let to_ply = self.game.state().ply().get();
                self.tell(|_, _| Some(accepted.clone()));
                self.tell(|_, _| Some(ServerMsg::TookBack { to_ply }));
            }
            OfferKind::Rematch => {
                let rematch = RoomId::new();
                let seed = Seed(u64::from_le_bytes(bytes()));
                write(games::rematch(&self.pool, &self.id, &rematch, seed)).await?;
                let room = rematch.to_string();
                self.tell(|_, _| Some(accepted.clone()));
                self.tell(|_, _| Some(ServerMsg::RematchRoom { room: room.clone() }));
            }
        }
        Ok(true)
    }

    /// How many of the game's moves are left once `player`'s last one is taken back, if they've
    /// made one: the dealt opening can't be.
    fn taken_back(&self, player: Player) -> Option<usize> {
        let dealt = self.game.opening().len();
        (0..self.game.moves().len())
            .rev()
            .find(|&index| ply(dealt + index).player() == player)
    }

    /// Puts `kind`, from `by`, to the other player, lapsing at `lapses` if it does.
    fn propose(&mut self, kind: OfferKind, by: Player, lapses: Option<Instant>) {
        let offer = Offer { kind, by };
        self.offer = Some(Pending { offer, lapses });
        self.tell(|_, _| Some(ServerMsg::Offer(offer)));
    }

    /// The waiting offer stands no more: it lapsed, or a move withdrew it.
    fn withdraw(&mut self) {
        let Some(pending) = self.offer.take() else {
            return;
        };
        let kind = pending.offer.kind;
        self.tell(|_, _| {
            Some(ServerMsg::OfferAnswered {
                kind,
                accepted: false,
            })
        });
    }

    /// Ends the game where it stands: a resignation or an agreed draw.
    async fn finish(&mut self, end: GameEnd) -> Result<(), Broken> {
        let moves = self.game.moves().len();
        write(games::end(&self.pool, &self.id, moves, end)).await?;
        self.stopped = Some(end);
        self.offer = None;
        self.tell(|_, _| Some(ServerMsg::Ended(end)));
        Ok(())
    }

    /// How the game ended, once it has.
    fn end(&self) -> Option<GameEnd> {
        let state = self.game.state();
        self.stopped.or(match state.status() {
            Status::Over(outcome) => Some(GameEnd::of(outcome, &state.ruleset())),
            Status::ToMove { .. } => None,
        })
    }

    fn snapshot(&self) -> Snapshot {
        Snapshot {
            record: self.game.record(),
            players: self.seats.players(),
            takebacks: self.takebacks,
            offer: self.offer.as_ref().map(|pending| pending.offer),
            draws: self.draws,
            end: self.end(),
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

/// The ply `index` counts to, the dealt opening included.
fn ply(index: usize) -> Ply {
    Ply::new(u16::try_from(index).expect("a game has at most 729 plies"))
}

/// Resolves at `at`, or never without one.
async fn until(at: Option<Instant>) {
    match at {
        Some(at) => tokio::time::sleep_until(at).await,
        None => std::future::pending().await,
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
