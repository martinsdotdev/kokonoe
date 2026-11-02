//! `GET /api/socket`, the game socket (spec §10.3). It opens with a `Hello` naming the build and
//! the Room, then carries the socket's moves to the Room and the Room's messages back.

use std::time::Duration;

use axum::extract::State;
use axum::extract::ws::{CloseFrame, Message, WebSocket, WebSocketUpgrade, close_code};
use axum::http::HeaderMap;
use axum::response::Response;
use kokonoe_engine::RULES_VERSION;
use kokonoe_protocol::{ClientMsg, CloseCode, PROTOCOL, ServerMsg};
use tokio::sync::mpsc;

use crate::room::{Act, Command, Conn, RoomId};
use crate::rooms::{Joined, Refused};
use crate::session::{self, Session};
use crate::time;
use crate::{AppState, Unavailable, from_here, not_from_here};

/// How long a socket has to say `Hello`.
const HELLO_TIMEOUT: Duration = Duration::from_secs(10);

/// How often the server pings an open socket, so proxies and phones don't drop it as idle while
/// a player thinks.
const KEEPALIVE: Duration = Duration::from_secs(30);

/// How many messages wait for a slow socket before its Room drops it.
const QUEUE: usize = 32;

/// The largest message a client may send: a `Hello` or a `Move` is far smaller.
const MAX_MESSAGE: usize = 4096;

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
    let Joined { conn, room } = match hello(socket, state, session, out).await {
        Ok(joined) => joined,
        Err(ending) => return ending,
    };
    let ending = relay(socket, &mut inbox, conn, &room).await;
    let _ = room.send(Command::Leave { conn }).await;
    ending
}

/// Reads the socket's `Hello`, and seats it in the Room it names.
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
    let room = match serde_json::from_str(&text) {
        Ok(ClientMsg::Hello {
            protocol,
            rules_version,
            room,
            ..
        }) => {
            if protocol != PROTOCOL || rules_version != RULES_VERSION {
                return Err((CloseCode::UpdateRequired.into(), "another build"));
            }
            room
        }
        Ok(_) => return Err((close_code::POLICY, "hello first")),
        Err(_) => return Err((close_code::INVALID, "not a message")),
    };
    let no_room = (CloseCode::NoRoom.into(), "no such Room");
    let room = RoomId::parse(&room).ok_or(no_room)?;
    let joined = state
        .rooms
        .join(&room, session.account, &session.handle, out);
    joined.await.map_err(|refused| match refused {
        Refused::NoRoom => no_room,
        Refused::Full => (CloseCode::Forbidden.into(), "full"),
        Refused::Unavailable => (close_code::ERROR, "unavailable"),
    })
}

/// Carries the socket's moves to its Room, as `conn`, and the Room's messages from `inbox` back,
/// until either side ends it.
async fn relay(
    socket: &mut WebSocket,
    inbox: &mut mpsc::Receiver<ServerMsg>,
    conn: Conn,
    room: &mpsc::Sender<Command>,
) -> Ending {
    let start = tokio::time::Instant::now() + KEEPALIVE;
    let mut keepalive = tokio::time::interval_at(start, KEEPALIVE);
    loop {
        tokio::select! {
            received = socket.recv() => match received {
                Some(Ok(Message::Text(text))) => match serde_json::from_str(&text) {
                    // Answered here, so the server's time isn't held up behind the Room's work.
                    Ok(ClientMsg::Ping { t }) => {
                        let pong = ServerMsg::Pong { t, server_ms: time::unix_ms() };
                        let text = serde_json::to_string(&pong).expect("a message is always JSON");
                        if socket.send(Message::Text(text.into())).await.is_err() {
                            return GONE;
                        }
                    }
                    Ok(message) => match Act::of(message) {
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
            },
            sent = inbox.recv() => match sent {
                Some(message) => {
                    let text = serde_json::to_string(&message).expect("a message is always JSON");
                    if socket.send(Message::Text(text.into())).await.is_err() {
                        return GONE;
                    }
                }
                // The Room dropped this socket, or stopped: the page reconnects for a fresh
                // Welcome.
                None => return (close_code::ERROR, "reconnect"),
            },
            _ = keepalive.tick() => {
                if socket.send(Message::Ping(Default::default())).await.is_err() {
                    return GONE;
                }
            }
        }
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
