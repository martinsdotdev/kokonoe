//! A Room in play: one task owns its Game, its seats and its sockets, and takes their commands in
//! turn, so two moves can never race. It writes each move, ending and takeback to Postgres before
//! telling anyone, and when a write fails it stops, closing every socket, rather than guess what
//! was stored: a write that timed out may still have landed. It also stops once its last socket
//! leaves. Either way the next `Hello` loads the Room again from the table.
//!
//! It also keeps spec §10.2's policies, and at most one offer waiting, which a stopped Room forgets,
//! and a timed Room's clocks (spec §11): a turn past its deadline aborts the game or loses it on
//! time, before anything else the Room hears, even when the Room loads again long after.
//!
//! And it keeps who is in it (spec §10.3 and §11): a seated player with no socket here is away,
//! which the table holds and the other player hears. Once they've been away long enough the other
//! may Claim the game, and a game both players left ends Abandoned a day on.

use std::collections::HashMap;
use std::fmt;
use std::time::Duration;

use kokonoe_engine::{Cell, Game, PlayError, Played, Player, Ply, Seed, Status};
use kokonoe_protocol::{
    Abort, Away, ClaimKind, ClientMsg, Clocks, Draws, GameEnd, GameResult, Offer, OfferKind,
    Presence, Rejection, ServerMsg, Snapshot, Termination,
};
use sqlx::PgPool;
use tokio::sync::{mpsc, oneshot};
use tokio::time::Instant;

use crate::clock::{Clock, Deadline};
use crate::games::{self, Seat, Seats, Stored};
use crate::random::{bytes, hex};
use crate::session::AccountId;
use crate::time::{long_ms, ms, unix_ms};

/// How long a write may take before the Room gives up on it.
const WRITE_TIMEOUT: Duration = Duration::from_secs(2);

/// How many commands wait for a Room before its sockets wait too.
const COMMANDS: usize = 64;

/// How long things take in a Room: how long an offer waits for its answer (spec §10.2), which the
/// players hear as each `Offer`'s `lapses_in`; a timed Room's first-move window and the units of
/// its time controls, how long a player is away before a Claim and before an empty Room is
/// abandoned (spec §11); and how often the server looks for such Rooms and pings a socket. Tests
/// shorten them. A draw offer has no deadline: it stands until it's answered or the offerer
/// moves.
#[derive(Clone, Copy, Debug)]
pub struct Timing {
    /// How long a takeback waits for its answer.
    pub takeback: Duration,
    /// How long a rematch waits for its answer.
    pub rematch: Duration,
    /// How long a player has for their first move in a timed Room before the game aborts.
    pub abort: Duration,
    /// A time control's minute, its bank's unit.
    pub minute: Duration,
    /// A time control's second, its increment's unit.
    pub second: Duration,
    /// How long a seated player is away before the other may Claim the game.
    pub claim: Duration,
    /// How long a game both players are away from waits before it ends Abandoned.
    pub abandon: Duration,
    /// How often the server looks for Rooms to abandon. Not zero.
    pub sweep: Duration,
    /// How often the server pings an open socket; one that answers neither of two in a row is
    /// closed. Not zero.
    pub keepalive: Duration,
}

impl Default for Timing {
    fn default() -> Timing {
        Timing {
            takeback: Duration::from_secs(15),
            rematch: Duration::from_secs(60),
            abort: Duration::from_secs(30),
            minute: Duration::from_secs(60),
            second: Duration::from_secs(1),
            claim: Duration::from_secs(60),
            abandon: Duration::from_secs(24 * 60 * 60),
            sweep: Duration::from_secs(60),
            keepalive: Duration::from_secs(30),
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
    Act {
        conn: Conn,
        act: Act,
    },
    Leave {
        conn: Conn,
    },
    /// Nothing: a Room loaded only to settle what's due, which then stops if no one is in it.
    Wake,
}

/// What a seated socket asks of the Room.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Act {
    Move { ply: u16, cell: u16 },
    Resign,
    Offer(OfferKind),
    Answer(OfferKind, bool),
    Claim(ClaimKind),
}

impl Act {
    /// What `message` asks, or nothing for what isn't the Room's: a `Hello`, which only opens a
    /// socket, or a `Ping`, which the socket answers itself.
    pub fn of(message: ClientMsg) -> Option<Act> {
        Some(match message {
            ClientMsg::Hello { .. } | ClientMsg::Ping { .. } => return None,
            ClientMsg::Move { ply, cell } => Act::Move { ply, cell },
            ClientMsg::Resign => Act::Resign,
            ClientMsg::OfferDraw => Act::Offer(OfferKind::Draw),
            ClientMsg::AskTakeback => Act::Offer(OfferKind::Takeback),
            ClientMsg::OfferRematch => Act::Offer(OfferKind::Rematch),
            ClientMsg::AnswerDraw { accept } => Act::Answer(OfferKind::Draw, accept),
            ClientMsg::AnswerTakeback { accept } => Act::Answer(OfferKind::Takeback, accept),
            ClientMsg::AnswerRematch { accept } => Act::Answer(OfferKind::Rematch, accept),
            ClientMsg::Claim { kind } => Act::Claim(kind),
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

/// Why a move wasn't played: the game is over, or the move is refused.
enum Refused {
    Over(GameEnd),
    Rejected(Rejection),
}

impl From<Refused> for ServerMsg {
    fn from(refused: Refused) -> ServerMsg {
        match refused {
            Refused::Over(end) => ServerMsg::Ended(end),
            Refused::Rejected(reason) => ServerMsg::Rejected { reason },
        }
    }
}

struct Room {
    id: RoomId,
    pool: PgPool,
    timing: Timing,
    game: Game,
    seats: Seats,
    takebacks: bool,
    draws: Draws,
    /// How the game ended other than by its rules: a resignation, an agreed draw, a loss on time,
    /// a Claim or an abandonment.
    stopped: Option<GameEnd>,
    offer: Option<Pending>,
    /// The Room this game's accepted rematch made.
    rematch: Option<RoomId>,
    /// A timed Room's clocks.
    clock: Option<Clock>,
    /// When the turn now running began.
    since: Instant,
    sockets: HashMap<Conn, Socket>,
    next: u64,
    /// Since when each seated player has had no socket here, as the table holds it.
    absent: Absent,
    /// The game was aborted, and its row is gone.
    gone: bool,
}

/// Since when each seated player has had no socket in the Room, in the server's Unix ms: None while
/// they have one, and for an empty seat. Wall time, as the wire and the table count it.
#[derive(Clone, Copy, Debug, Default)]
struct Absent {
    x: Option<u64>,
    o: Option<u64>,
}

impl Absent {
    fn get(self, player: Player) -> Option<u64> {
        match player {
            Player::X => self.x,
            Player::O => self.o,
        }
    }

    fn set(&mut self, player: Player, since: Option<u64>) {
        match player {
            Player::X => self.x = since,
            Player::O => self.o = since,
        }
    }
}

/// The offer waiting for an answer, and when it lapses.
struct Pending {
    kind: OfferKind,
    by: Player,
    lapses: Option<Instant>,
}

impl Pending {
    /// The offer as the players hear it at `now`, with the time it has left.
    fn offer(&self, now: Instant) -> Offer {
        Offer {
            kind: self.kind,
            by: self.by,
            lapses_in: self.lapses.map(|at| ms(at.saturating_duration_since(now))),
        }
    }
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
    // A rules ending is the engine's to tell again; only these need the table.
    let stopped = stored.end.filter(|end| {
        matches!(
            end.termination,
            Termination::Resign
                | Termination::AgreedDraw
                | Termination::Timeout
                | Termination::Claim
                | Termination::Abandoned
        )
    });
    let clock = stored.clock.map(|control| {
        let first = first_mover(&game);
        let mut clock = Clock::new(control, timing.minute, timing.second, first, &stored.clocks);
        if let Some(GameEnd {
            result: Some(result),
            termination: Termination::Timeout,
        }) = stopped
        {
            clock.flag(loser(result));
        }
        clock
    });
    let unix = unix_ms();
    let since = |ago: Option<Duration>| ago.map(|ago| unix.saturating_sub(long_ms(ago)));
    let absent = Absent {
        x: since(stored.x_away),
        o: since(stored.o_away),
    };
    let now = Instant::now();
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
        rematch: stored.rematch,
        clock,
        since: now.checked_sub(stored.turn).unwrap_or(now),
        sockets: HashMap::new(),
        next: 0,
        absent,
        gone: false,
    };
    tokio::spawn(room.run(inbox));
    commands
}

impl Room {
    async fn run(mut self, inbox: mpsc::Receiver<Command>) {
        if self.serve(inbox).await.is_err() {
            tracing::error!(room = %self.id, "a Room stopped: what's stored is unknown");
        }
    }

    /// Plays the Room until its last socket leaves, or a write leaves what's stored unknown.
    async fn serve(&mut self, mut inbox: mpsc::Receiver<Command>) -> Result<(), Broken> {
        // A Room just loaded has no one in it, though the table may hold someone here: a task an
        // earlier write stopped left them so.
        self.present().await?;
        loop {
            // A Room loaded long after a deadline or its wait settles it before the message that
            // loaded it, which a timer for a moment already past wouldn't.
            self.settle().await?;
            // An aborted game's row is gone, so a socket still to come finds no Room.
            if self.gone {
                return Ok(());
            }
            let due = self.due();
            let lapses = self.offer.as_ref().and_then(|pending| pending.lapses);
            tokio::select! {
                // A turn past its deadline or a game left past its wait, then an offer past its
                // lapse, is settled before any message that came after it.
                biased;
                () = until(due) => self.settle().await?,
                () = until(lapses) => self.withdraw(),
                command = inbox.recv() => match command {
                    Some(Command::Join(join)) => self.join(join).await?,
                    Some(Command::Act { conn, act }) => self.act(conn, act).await?,
                    Some(Command::Leave { conn }) => {
                        self.sockets.remove(&conn);
                    }
                    Some(Command::Wake) => {}
                    None => return Ok(()),
                },
            }
            // Whoever came or went, written and told, before the Room waits again.
            self.present().await?;
            if self.sockets.is_empty() {
                return Ok(());
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
        if seated && self.seats.full() {
            // The game begins, and with it the first turn. A player gone before it began is gone
            // from it only from now, as the table has it.
            self.since = Instant::now();
            let other = player.opponent();
            if self.absent.get(other).is_some() {
                self.absent.set(other, Some(unix_ms()));
            }
        }
        let conn = Conn(self.next);
        self.next += 1;
        self.sockets.insert(conn, Socket { player, out });
        let _ = reply.send(Some(conn));
        // A seat that fills tells everyone, so whoever was waiting learns who joined.
        let snapshot = self.snapshot();
        self.tell(|other, socket| {
            (seated || other == conn).then(|| ServerMsg::Welcome {
                you: Some(socket.player),
                snapshot: snapshot.clone(),
            })
        });
        Ok(())
    }

    /// Does what a seated socket asked, or tells it why not.
    async fn act(&mut self, conn: Conn, act: Act) -> Result<(), Broken> {
        self.expire().await?;
        let Some(player) = self.sockets.get(&conn).map(|socket| socket.player) else {
            return Ok(());
        };
        let allowed = match act {
            Act::Move { ply, cell } => return self.play(conn, player, ply, cell).await,
            Act::Resign => self.resign(player).await?,
            Act::Offer(kind) => self.offer(player, kind).await?,
            Act::Answer(kind, accept) => self.answer(player, kind, accept).await?,
            Act::Claim(kind) => self.claim(player, kind).await?,
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
        match self.judge(player, ply, cell) {
            Ok(true) => {}
            Ok(false) => return Ok(()),
            Err(refused) => {
                self.send(conn, refused.into());
                return Ok(());
            }
        }
        let elapsed = self.since.elapsed();
        let left = self.clock.as_mut().map(|clock| ms(clock.spend(elapsed)));
        let ended = self.end();
        let before = self.game.moves().len() - 1;
        write(games::append(
            &self.pool, &self.id, before, cell, left, ended,
        ))
        .await?;
        let now = Instant::now();
        self.since = now;
        let clocks = self.clocks(now);
        self.announce(ServerMsg::Moved { ply, cell, clocks });
        if let Some(end) = ended {
            self.offer = None;
            self.announce(ServerMsg::Ended(end));
            return Ok(());
        }
        // A move withdraws a takeback, which was asked of the game before it, and the mover's own
        // draw offer.
        let withdrawn = self.offer.as_ref().is_some_and(|pending| {
            let &Pending { kind, by, .. } = pending;
            kind == OfferKind::Takeback || (kind == OfferKind::Draw && by == player)
        });
        if withdrawn {
            self.withdraw();
        }
        Ok(())
    }

    /// Plays `player`'s move at `ply` if it's theirs to make and the rules allow it: true once
    /// it's played, false for a move already made, or why it's refused.
    fn judge(&mut self, player: Player, ply: u16, cell: u16) -> Result<bool, Refused> {
        let refuse = Refused::Rejected;
        if let Some(end) = self.end() {
            return Err(Refused::Over(end));
        }
        if !self.seats.full() || Ply::new(ply).player() != player {
            return Err(refuse(Rejection::NotAllowed));
        }
        let depth = self.game.state().ruleset().depth;
        let cell = Cell::from_index(depth, cell).map_err(|_| refuse(Rejection::IllegalMove))?;
        match self.game.play(Ply::new(ply), cell) {
            Ok(Played::Now(_)) => Ok(true),
            Ok(Played::Already) => Ok(false),
            Err(PlayError::Illegal(_)) => Err(refuse(Rejection::IllegalMove)),
            Err(PlayError::OutOfOrder { want, .. }) => {
                Err(refuse(Rejection::OutOfOrder { want: want.get() }))
            }
        }
    }

    async fn resign(&mut self, player: Player) -> Result<bool, Broken> {
        if !self.seats.full() || self.end().is_some() {
            return Ok(false);
        }
        let end = GameEnd {
            result: Some(GameResult::won_by(Some(player.opponent()))),
            termination: Termination::Resign,
        };
        self.stop(end).await?;
        self.announce(ServerMsg::Ended(end));
        Ok(true)
    }

    /// Ends the game as `player` claims it, once the other player has been away long enough: a
    /// claimed win is an ordinary loss for them. A clock that ran out while they were away has
    /// already ended it on time, since `act` settles that first.
    async fn claim(&mut self, player: Player, kind: ClaimKind) -> Result<bool, Broken> {
        let away = self.away(player.opponent(), unix_ms());
        if !self.turn() || !away.is_some_and(|away| away.claims_in == 0) {
            return Ok(false);
        }
        let end = GameEnd {
            result: Some(claimed(player, kind)),
            termination: Termination::Claim,
        };
        self.stop(end).await?;
        self.announce(ServerMsg::Ended(end));
        Ok(true)
    }

    /// Puts `player`'s offer of `kind` to the other player, if it can be made now.
    async fn offer(&mut self, player: Player, kind: OfferKind) -> Result<bool, Broken> {
        let wait = match kind {
            OfferKind::Draw => {
                let made = self.draws.get(player);
                if !self.can_offer(false) || made >= Draws::MAX {
                    return Ok(false);
                }
                write(games::offer_draw(&self.pool, &self.id, player, made)).await?;
                self.draws.bump(player);
                None
            }
            OfferKind::Takeback => {
                let ours = self.taken_back(player).is_some();
                if !self.takebacks || !self.can_offer(false) || !ours {
                    return Ok(false);
                }
                Some(self.timing.takeback)
            }
            OfferKind::Rematch => {
                if !self.can_offer(true) || self.rematch.is_some() {
                    return Ok(false);
                }
                Some(self.timing.rematch)
            }
        };
        let now = Instant::now();
        let pending = Pending {
            kind,
            by: player,
            lapses: wait.map(|wait| now + wait),
        };
        self.announce(ServerMsg::Offer(pending.offer(now)));
        self.offer = Some(pending);
        Ok(true)
    }

    /// Both seats are filled, the game is `over` or goes on, and no offer waits.
    fn can_offer(&self, over: bool) -> bool {
        self.seats.full() && self.end().is_some() == over && self.offer.is_none()
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
            .filter(|pending| pending.kind == kind && pending.by != player);
        let Some(by) = waiting.map(|pending| pending.by) else {
            return Ok(false);
        };
        if !accept {
            self.withdraw();
            return Ok(true);
        }
        let accepted = ServerMsg::OfferAnswered {
            kind,
            accepted: true,
        };
        match kind {
            OfferKind::Draw => self.agree_draw(accepted).await,
            OfferKind::Takeback => self.take_back(by, accepted).await,
            OfferKind::Rematch => self.rematch(accepted).await,
        }
    }

    async fn agree_draw(&mut self, accepted: ServerMsg) -> Result<bool, Broken> {
        let end = GameEnd {
            result: Some(GameResult::Draw),
            termination: Termination::AgreedDraw,
        };
        self.stop(end).await?;
        self.announce(accepted);
        self.announce(ServerMsg::Ended(end));
        Ok(true)
    }

    /// Undoes back to before `asker`'s last move.
    async fn take_back(&mut self, asker: Player, accepted: ServerMsg) -> Result<bool, Broken> {
        // Nothing moved since it was asked, or the move would have withdrawn it.
        let Some(keep) = self.taken_back(asker) else {
            return Ok(false);
        };
        let before = self.game.moves().len();
        write(games::take_back(&self.pool, &self.id, before, keep)).await?;
        for _ in keep..before {
            self.game.undo();
        }
        if let Some(clock) = &mut self.clock {
            clock.take_back(keep);
        }
        let now = Instant::now();
        self.since = now;
        self.offer = None;
        let to_ply = self.game.state().ply().get();
        let clocks = self.clocks(now);
        self.announce(accepted);
        self.announce(ServerMsg::TookBack { to_ply, clocks });
        Ok(true)
    }

    /// Makes the next Room, X and O swapped, and sends both players to it.
    async fn rematch(&mut self, accepted: ServerMsg) -> Result<bool, Broken> {
        let rematch = RoomId::new();
        let seed = Seed(u64::from_le_bytes(bytes()));
        write(games::rematch(&self.pool, &self.id, &rematch, seed)).await?;
        self.offer = None;
        let room = rematch.to_string();
        self.rematch = Some(rematch);
        self.announce(accepted);
        self.announce(ServerMsg::RematchRoom { room });
        Ok(true)
    }

    fn taken_back(&self, player: Player) -> Option<usize> {
        taken_back(&self.game, player)
    }

    /// The waiting offer stands no more: declined, lapsed, or withdrawn by a move.
    fn withdraw(&mut self) {
        let Some(pending) = self.offer.take() else {
            return;
        };
        let kind = pending.kind;
        self.announce(ServerMsg::OfferAnswered {
            kind,
            accepted: false,
        });
    }

    /// Ends the game where it stands, a resignation, an agreed draw, a timeout, a Claim or an
    /// abandonment, in the table first. Any offer waiting goes with it, as `Ended` tells the
    /// players.
    async fn stop(&mut self, end: GameEnd) -> Result<(), Broken> {
        let moves = self.game.moves().len();
        write(games::end(&self.pool, &self.id, moves, end)).await?;
        self.stopped = Some(end);
        self.offer = None;
        Ok(())
    }

    /// A turn runs: both seats are filled and the game goes on, neither ended nor aborted.
    fn turn(&self) -> bool {
        self.seats.full() && self.end().is_none() && !self.gone
    }

    /// Settles what has come due: a turn past its deadline, which aborts the game or loses it on
    /// time, or a game both players left past its wait, which ends Abandoned.
    async fn settle(&mut self) -> Result<(), Broken> {
        self.expire().await?;
        if self.abandons().is_some_and(|at| at <= Instant::now()) {
            self.stop(GameEnd::ABANDONED).await?;
        }
        Ok(())
    }

    /// When `settle` next has something to settle: the turn's deadline, or the game's abandonment.
    fn due(&self) -> Option<Instant> {
        let deadline = self.deadline().map(|(at, _)| at);
        [deadline, self.abandons()].into_iter().flatten().min()
    }

    /// When a game both players are away from ends Abandoned: the Room's wait after the later one
    /// left.
    fn abandons(&self) -> Option<Instant> {
        if !self.turn() || !self.sockets.is_empty() {
            return None;
        }
        abandons_at(self.absent.x, self.absent.o, self.timing.abandon).map(instant)
    }

    /// Whether `player` has a socket here.
    fn here(&self, player: Player) -> bool {
        self.sockets.values().any(|socket| socket.player == player)
    }

    /// `player`'s absence as the other player hears it at `now`, while they have no socket here.
    fn away(&self, player: Player, now: u64) -> Option<Away> {
        let since = self.absent.get(player).filter(|_| !self.here(player))?;
        Some(Away {
            since,
            claims_in: claims_in(since, self.timing.claim, now),
        })
    }

    /// Writes and tells whoever came or went: a seated player whose first socket arrived, or whose
    /// last went, by a `Leave`, a dropped queue or an abort. Telling can drop a socket, so it looks
    /// again until no one changes.
    async fn present(&mut self) -> Result<(), Broken> {
        // An aborted game's row is gone, and so is everyone in it.
        if self.gone {
            return Ok(());
        }
        loop {
            let changed = [Player::X, Player::O].into_iter().find(|&player| {
                self.seats.get(player).is_some()
                    && self.here(player) == self.absent.get(player).is_some()
            });
            let Some(player) = changed else {
                return Ok(());
            };
            let left = !self.here(player);
            write(games::away(&self.pool, &self.id, player, left)).await?;
            let now = unix_ms();
            self.absent.set(player, left.then_some(now));
            let away = self.away(player, now);
            self.tell(|_, socket| {
                (socket.player != player).then_some(ServerMsg::Presence { player, away })
            });
        }
    }

    /// When the turn now running runs out, in a timed Room, and how.
    fn deadline(&self) -> Option<(Instant, Deadline)> {
        let clock = self.clock.as_ref().filter(|_| self.turn())?;
        let deadline = clock.deadline(self.timing.abort);
        Some((self.since + deadline.after(), deadline))
    }

    /// Settles the turn now running if it has run out: before its player's first move the game
    /// aborts, and the Room is gone, its sockets closed; after it, they lose on time.
    async fn expire(&mut self) -> Result<(), Broken> {
        let Some((at, deadline)) = self.deadline() else {
            return Ok(());
        };
        let Some(clock) = self.clock.as_mut().filter(|_| at <= Instant::now()) else {
            return Ok(());
        };
        let player = clock.to_move();
        match deadline {
            Deadline::Abort(_) => {
                let moves = self.game.moves().len();
                write(games::abort(&self.pool, &self.id, moves)).await?;
                let reason = Abort::FirstMoveTimeout;
                self.announce(ServerMsg::Aborted { reason });
                self.gone = true;
                self.sockets.clear();
            }
            Deadline::Flag(_) => {
                clock.flag(player);
                let end = GameEnd {
                    result: Some(GameResult::won_by(Some(player.opponent()))),
                    termination: Termination::Timeout,
                };
                self.stop(end).await?;
                self.announce(ServerMsg::Ended(end));
            }
        }
        Ok(())
    }

    /// A timed Room's clocks at `now`: the running one counted down, and a player who lost on time
    /// at 0.
    fn clocks(&self, now: Instant) -> Option<Clocks> {
        let clock = self.clock.as_ref()?;
        let elapsed = now.saturating_duration_since(self.since);
        let reading = clock.read(self.turn(), elapsed);
        let at = unix_ms();
        let aborts_at = match self.deadline() {
            Some((_, Deadline::Abort(after))) => {
                Some(at.saturating_add(u64::from(ms(after.saturating_sub(elapsed)))))
            }
            _ => None,
        };
        Some(Clocks {
            x_ms: ms(reading.x),
            o_ms: ms(reading.o),
            running: reading.running,
            at,
            aborts_at,
        })
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
        let now = unix_ms();
        Snapshot {
            record: self.game.record(),
            players: self.seats.players(),
            takebacks: self.takebacks,
            offer: self
                .offer
                .as_ref()
                .map(|pending| pending.offer(Instant::now())),
            draws: self.draws,
            end: self.end(),
            rematch: self.rematch.as_ref().map(RoomId::to_string),
            clock: self.clock.as_ref().map(Clock::control),
            clocks: self.clocks(Instant::now()),
            presence: Presence {
                x: self.away(Player::X, now),
                o: self.away(Player::O, now),
            },
            watching: 0,
            delay_ms: 0,
        }
    }

    /// Queues `message` for every socket.
    fn announce(&mut self, message: ServerMsg) {
        self.tell(|_, _| Some(message.clone()));
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

/// How many of `game`'s moves are left once `player`'s last one is taken back, if they've made
/// one: the dealt opening can't be.
fn taken_back(game: &Game, player: Player) -> Option<usize> {
    let dealt = game.opening().len();
    (0..game.moves().len())
        .rev()
        .find(|&index| ply(dealt + index).player() == player)
}

/// Who makes `game`'s first played move, after its dealt opening.
fn first_mover(game: &Game) -> Player {
    ply(game.opening().len()).player()
}

/// How many ms from `now` before a player away `since` can be claimed against: 0 once they can.
fn claims_in(since: u64, wait: Duration, now: u64) -> u32 {
    let due = since.saturating_add(long_ms(wait));
    u32::try_from(due.saturating_sub(now)).unwrap_or(u32::MAX)
}

/// When a game ends Abandoned, both its players away: `wait` after the later one left, in Unix
/// ms. Never while either is here.
fn abandons_at(x: Option<u64>, o: Option<u64>, wait: Duration) -> Option<u64> {
    Some(x?.max(o?).saturating_add(long_ms(wait)))
}

/// The Result of a game `player` claims as `kind`.
fn claimed(player: Player, kind: ClaimKind) -> GameResult {
    match kind {
        ClaimKind::Win => GameResult::won_by(Some(player)),
        ClaimKind::Draw => GameResult::Draw,
    }
}

/// The moment the server's clock reaches `at`, in Unix ms, as the Room sleeps until it.
fn instant(at: u64) -> Instant {
    Instant::now() + Duration::from_millis(at.saturating_sub(unix_ms()))
}

/// The player who lost a game that ended `result` on time.
fn loser(result: GameResult) -> Player {
    match result {
        GameResult::XWins => Player::O,
        GameResult::OWins | GameResult::Draw => Player::X,
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
    use kokonoe_engine::{Preset, RandomPlies, Ruleset};

    use super::*;

    /// Plays the first legal Cell for whoever is to move.
    fn step(game: &mut Game) {
        let Status::ToMove { moves, .. } = game.state().status() else {
            panic!("the game is over");
        };
        game.play(game.state().ply(), moves.nth(0)).unwrap();
    }

    #[test]
    fn the_played_moves_start_after_a_dealt_opening() {
        assert_eq!(
            first_mover(&Game::new(Preset::Standard.ruleset(), Seed(1))),
            Player::X
        );
        for (dealt, first) in [(1, Player::O), (2, Player::X), (3, Player::O)] {
            let ruleset = Ruleset {
                opening: kokonoe_engine::Opening::Random(RandomPlies::try_from(dealt).unwrap()),
                ..Preset::Standard.ruleset()
            };
            let game = Game::new(ruleset, Seed(1));
            assert_eq!(first_mover(&game), first, "{dealt} dealt");
            let Status::ToMove { player, .. } = game.state().status() else {
                panic!("the game is over");
            };
            assert_eq!(player, first, "the engine agrees");
        }
    }

    #[test]
    fn a_takeback_never_reaches_into_a_dealt_opening() {
        let ruleset = Ruleset {
            opening: kokonoe_engine::Opening::Random(RandomPlies::try_from(3).unwrap()),
            ..Preset::Standard.ruleset()
        };
        // Three dealt plies, X, O and X: O moves first.
        let mut game = Game::new(ruleset, Seed(1));
        assert_eq!(game.opening().len(), 3);
        assert_eq!(
            (taken_back(&game, Player::X), taken_back(&game, Player::O)),
            (None, None)
        );
        step(&mut game);
        assert_eq!(
            taken_back(&game, Player::O),
            Some(0),
            "back to the dealt opening"
        );
        assert_eq!(taken_back(&game, Player::X), None, "X has dealt plies only");
        step(&mut game);
        assert_eq!(taken_back(&game, Player::X), Some(1));
        assert_eq!(taken_back(&game, Player::O), Some(0), "two plies back");
    }

    #[test]
    fn offers_wait_as_spec_10_2_says() {
        let timing = Timing::default();
        assert_eq!(timing.takeback, Duration::from_secs(15));
        assert_eq!(timing.rematch, Duration::from_secs(60));
    }

    #[test]
    fn presence_waits_as_spec_11_says() {
        let timing = Timing::default();
        assert_eq!(timing.claim, Duration::from_secs(60));
        assert_eq!(timing.abandon, Duration::from_secs(24 * 60 * 60));
        assert_eq!(timing.keepalive, Duration::from_secs(30));
    }

    #[test]
    fn a_claim_comes_due_its_wait_after_the_other_player_left() {
        let wait = Duration::from_secs(60);
        let since = 1_793_600_000_000;
        assert_eq!(claims_in(since, wait, since), 60_000);
        assert_eq!(claims_in(since, wait, since + 59_999), 1);
        assert_eq!(claims_in(since, wait, since + 60_000), 0);
        assert_eq!(claims_in(since, wait, since + 3_600_000), 0, "never less");
    }

    #[test]
    fn a_claim_wins_or_draws_for_its_claimant() {
        assert_eq!(claimed(Player::O, ClaimKind::Win), GameResult::OWins);
        assert_eq!(claimed(Player::X, ClaimKind::Win), GameResult::XWins);
        assert_eq!(claimed(Player::X, ClaimKind::Draw), GameResult::Draw);
    }

    #[test]
    fn a_game_is_abandoned_its_wait_after_the_later_player_left() {
        let wait = Duration::from_secs(24 * 60 * 60);
        assert_eq!(abandons_at(Some(5_000), None, wait), None, "O is here");
        assert_eq!(abandons_at(None, None, wait), None);
        assert_eq!(
            abandons_at(Some(5_000), Some(9_000), wait),
            Some(9_000 + 86_400_000)
        );
    }

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
