//! `GET /api/socket`, the game socket (spec §10.3). It opens with a `Hello` naming the build and
//! the Room, then carries the socket's moves to the Room and the Room's messages back: to a
//! Spectator of a game Spectators hear late, each that late (spec §10.2).

use std::collections::VecDeque;
use std::time::Duration;

use axum::extract::State;
use axum::extract::ws::{CloseFrame, Message, WebSocket, WebSocketUpgrade, close_code};
use axum::http::HeaderMap;
use axum::response::Response;
use kokonoe_engine::RULES_VERSION;
use kokonoe_protocol::{ClientMsg, CloseCode, PROTOCOL, Role, ServerMsg};
use tokio::sync::mpsc;
use tokio::time::Instant;

use crate::room::{Act, Command, Conn, RoomId, until};
use crate::rooms::{Joined, Refused};
use crate::session::{self, Session};
use crate::time;
use crate::{AppState, Unavailable, from_here, not_from_here};

/// How long a socket has to say `Hello`.
const HELLO_TIMEOUT: Duration = Duration::from_secs(10);

/// How many messages wait for a slow socket before its Room drops it.
const QUEUE: usize = 32;

/// The largest message a client may send: a `Hello` or a `Move` is far smaller.
const MAX_MESSAGE: usize = 4096;

/// How many of the Room's messages a socket that hears it late holds before it's closed: far more
/// than a game says in a Rated game's delay.
const HELD: usize = 256;

/// How the server ends a socket: the close code, and the reason sent with it.
type Ending = (u16, &'static str);

/// A frame the server couldn't read, such as one past [`MAX_MESSAGE`].
const UNREADABLE: Ending = (close_code::PROTOCOL, "unreadable");

/// The socket closed itself, or its connection went.
const GONE: Ending = (close_code::NORMAL, "");

/// One opened from another site, or without saying where it's from, is refused outright. One
/// without a session is accepted and then closed with 4003, since a page can't read why a refused
/// upgrade failed but can read a close code. It doesn't move the session's expiry: a socket has
/// no cookie to send back.
pub async fn socket(
    State(state): State<AppState>,
    headers: HeaderMap,
    upgrade: WebSocketUpgrade,
) -> Result<Response, Unavailable> {
    if !from_here(&headers, &state.origin) {
        return Ok(not_from_here());
    }
    let found = session::find(&state.pool, &headers).await?;
    let upgrade = upgrade.max_message_size(MAX_MESSAGE);
    Ok(upgrade.on_upgrade(move |mut socket| async move {
        let ending = match found {
            Some(session) => serve(&mut socket, &state, &session).await,
            None => (CloseCode::Forbidden.into(), "no session"),
        };
        let close = CloseFrame {
            code: ending.0,
            reason: ending.1.into(),
        };
        // A socket that closed itself takes no Close back, which is no matter.
        let _ = socket.send(Message::Close(Some(close))).await;
    }))
}

async fn serve(socket: &mut WebSocket, state: &AppState, session: &Session) -> Ending {
    let (out, mut inbox) = mpsc::channel(QUEUE);
    let Joined { conn, room, delay } = match hello(socket, state, session, out).await {
        Ok(joined) => joined,
        Err(ending) => return ending,
    };
    let ending = relay(socket, &mut inbox, conn, &room, state.keepalive, delay).await;
    let _ = room.send(Command::Leave { conn }).await;
    ending
}

/// Reads the socket's `Hello`, and brings it into the Room it names as the role it asks, or to the
/// seat its Account holds.
async fn hello(
    socket: &mut WebSocket,
    state: &AppState,
    session: &Session,
    out: mpsc::Sender<ServerMsg>,
) -> Result<Joined, Ending> {
    let text = match tokio::time::timeout(HELLO_TIMEOUT, next_message(socket)).await {
        Ok(Some(Ok(Message::Text(text)))) => text,
        Ok(Some(Ok(Message::Binary(_)))) => return Err((close_code::UNSUPPORTED, "text only")),
        Ok(Some(Err(_))) => return Err(UNREADABLE),
        Ok(_) => return Err(GONE),
        Err(_) => return Err((close_code::POLICY, "no hello")),
    };
    let (room, role): (String, Role) = match serde_json::from_str(&text) {
        Ok(ClientMsg::Hello {
            protocol,
            rules_version,
            room,
            role,
        }) => {
            if protocol != PROTOCOL || rules_version != RULES_VERSION {
                return Err((CloseCode::UpdateRequired.into(), "another build"));
            }
            (room, role)
        }
        Ok(_) => return Err((close_code::POLICY, "hello first")),
        Err(_) => return Err((close_code::INVALID, "not a message")),
    };
    let no_room = (CloseCode::NoRoom.into(), "no such Room");
    let room = RoomId::parse(&room).ok_or(no_room)?;
    let joined = state
        .rooms
        .join(&room, session.account, &session.handle, role, out);
    joined.await.map_err(|refused| match refused {
        Refused::NoRoom => no_room,
        Refused::Full => (CloseCode::Forbidden.into(), "full"),
        Refused::NoSpectators => (CloseCode::Forbidden.into(), "no spectators"),
        Refused::Unavailable => (close_code::ERROR, "unavailable"),
    })
}

/// Carries the socket's moves to its Room, as `conn`, and the Room's messages from `inbox` back,
/// each `delay` after the Room sent it, until either side ends it. It pings the socket every
/// `keepalive`, so proxies and phones don't drop it as idle while a player thinks, and closes one
/// that answers nothing to two pings in a row: a phone asleep with its socket still open, whose
/// player is then away.
async fn relay(
    socket: &mut WebSocket,
    inbox: &mut mpsc::Receiver<ServerMsg>,
    conn: Conn,
    room: &mpsc::Sender<Command>,
    keepalive: Duration,
    delay: Duration,
) -> Ending {
    let start = Instant::now() + keepalive;
    let mut pings = tokio::time::interval_at(start, keepalive);
    // A relay held up by a slow send mustn't fire its missed pings at once.
    pings.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    let mut unanswered = Unanswered::default();
    let mut held = Delayed::new(delay);
    // The Room still sends here: once it drops this socket or stops, what it sent before is still
    // to come, and nothing the socket asks reaches it.
    let mut open = true;
    loop {
        let due = held.due();
        let received = tokio::select! {
            received = socket.recv() => received,
            sent = inbox.recv(), if open => {
                match sent {
                    Some(message) if delay.is_zero() => {
                        if send(socket, &message).await.is_err() {
                            return GONE;
                        }
                    }
                    Some(message) => {
                        if !held.push(Instant::now(), message) {
                            return (close_code::ERROR, "reconnect");
                        }
                    }
                    // The Room dropped this socket, or stopped: the page reconnects for a fresh
                    // Welcome, once it has heard all the Room sent.
                    None if held.is_empty() => return (close_code::ERROR, "reconnect"),
                    None => open = false,
                }
                continue;
            }
            () = until(due) => {
                while let Some(message) = held.pop(Instant::now()) {
                    if send(socket, &message).await.is_err() {
                        return GONE;
                    }
                }
                if !open && held.is_empty() {
                    return (close_code::ERROR, "reconnect");
                }
                continue;
            }
            _ = pings.tick() => {
                if !unanswered.ping() {
                    return SILENT;
                }
                if socket.send(Message::Ping(Default::default())).await.is_err() {
                    return GONE;
                }
                continue;
            }
        };
        // Whatever it sends, a page that talks is there.
        if let Some(Ok(_)) = received {
            unanswered = Unanswered::default();
        }
        match received {
            Some(Ok(Message::Text(text))) => match serde_json::from_str(&text) {
                // Answered here, so the server's time isn't held up behind the Room's work.
                Ok(ClientMsg::Ping { t }) => {
                    let pong = ServerMsg::Pong {
                        t,
                        server_ms: time::unix_ms(),
                    };
                    let text = serde_json::to_string(&pong).expect("a message is always JSON");
                    if socket.send(Message::Text(text.into())).await.is_err() {
                        return GONE;
                    }
                }
                Ok(message) => match Act::of(message) {
                    Some(_) if !open => {}
                    Some(act) => {
                        if room.send(Command::Act { conn, act }).await.is_err() {
                            return (close_code::ERROR, "the Room stopped");
                        }
                    }
                    None => return (CloseCode::Forbidden.into(), "hello again"),
                },
                Err(_) => return (close_code::INVALID, "not a message"),
            },
            Some(Ok(Message::Binary(_))) => return (close_code::UNSUPPORTED, "text only"),
            Some(Ok(Message::Ping(_) | Message::Pong(_))) => {}
            Some(Err(_)) => return UNREADABLE,
            Some(Ok(Message::Close(_))) | None => return GONE,
        }
    }
}

/// Sends `message` on the socket, as JSON.
async fn send(socket: &mut WebSocket, message: &ServerMsg) -> Result<(), axum::Error> {
    let text = serde_json::to_string(message).expect("a message is always JSON");
    socket.send(Message::Text(text.into())).await
}

/// The Room's messages to a socket that hears it late, each held until `delay` after it came.
struct Delayed {
    delay: Duration,
    held: VecDeque<(Instant, ServerMsg)>,
}

impl Delayed {
    fn new(delay: Duration) -> Delayed {
        Delayed {
            delay,
            held: VecDeque::new(),
        }
    }

    /// Holds `message`, which came at `now`: false, holding nothing more, once [`HELD`] are.
    fn push(&mut self, now: Instant, message: ServerMsg) -> bool {
        if self.held.len() >= HELD {
            return false;
        }
        self.held.push_back((now + self.delay, message));
        true
    }

    /// When the next message is due, if one is held.
    fn due(&self) -> Option<Instant> {
        self.held.front().map(|&(at, _)| at)
    }

    /// The next message, if it's due by `now`.
    fn pop(&mut self, now: Instant) -> Option<ServerMsg> {
        if self.due()? > now {
            return None;
        }
        self.held.pop_front().map(|(_, message)| message)
    }

    fn is_empty(&self) -> bool {
        self.held.is_empty()
    }
}

/// A socket that answered neither of two pings in a row: a page asleep, or a network gone quiet.
const SILENT: Ending = (close_code::POLICY, "no answer");

/// How many pings in a row a socket has answered nothing to.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct Unanswered(u8);

impl Unanswered {
    /// One more ping is due: false once two in a row went unanswered, and the socket is closed.
    fn ping(&mut self) -> bool {
        if self.0 >= 2 {
            return false;
        }
        self.0 += 1;
        true
    }
}

/// The next message that isn't a ping or a pong, or what kept it from being read; nothing once
/// the socket closes.
async fn next_message(socket: &mut WebSocket) -> Option<Result<Message, axum::Error>> {
    loop {
        match socket.recv().await? {
            Ok(Message::Ping(_) | Message::Pong(_)) => {}
            read => return Some(read),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_late_socket_hears_each_message_its_delay_after_it_came() {
        let delay = Duration::from_millis(300);
        let mut held = Delayed::new(delay);
        let start = Instant::now();
        assert_eq!(held.due(), None);
        let moved = |ply| ServerMsg::Moved {
            ply,
            cell: 40,
            clocks: None,
        };
        assert!(held.push(start, moved(0)));
        assert!(held.push(start + Duration::from_millis(100), moved(1)));
        assert_eq!(held.due(), Some(start + delay));
        assert_eq!(
            held.pop(start + Duration::from_millis(299)),
            None,
            "not yet"
        );
        assert_eq!(held.pop(start + delay), Some(moved(0)));
        assert_eq!(held.pop(start + delay), None, "the next comes 100 ms on");
        assert_eq!(held.pop(start + Duration::from_millis(400)), Some(moved(1)));
        assert!(held.is_empty());
    }

    #[test]
    fn a_late_socket_holds_only_so_many() {
        let mut held = Delayed::new(Duration::from_secs(10));
        let now = Instant::now();
        let pong = ServerMsg::Pong { t: 0, server_ms: 0 };
        for _ in 0..HELD {
            assert!(held.push(now, pong.clone()));
        }
        assert!(!held.push(now, pong), "too far behind: the page reconnects");
    }

    #[test]
    fn two_unanswered_pings_close_a_socket() {
        let mut unanswered = Unanswered::default();
        assert!(unanswered.ping(), "the first ping goes out");
        assert!(unanswered.ping(), "and a second, the first unanswered");
        assert!(!unanswered.ping(), "neither answered: the socket closes");
        let mut answered = Unanswered::default();
        answered.ping();
        answered = Unanswered::default();
        assert!(
            answered.ping() && answered.ping(),
            "an answer starts the count again"
        );
    }
}
