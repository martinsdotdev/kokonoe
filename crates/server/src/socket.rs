//! `GET /api/socket`, the game socket (spec §10.3). It opens with a `Hello` naming the build and
//! the Room, then carries the socket's moves to the Room and the Room's messages back.

use std::time::Duration;

use axum::extract::State;
use axum::extract::ws::{CloseFrame, Message, WebSocket, WebSocketUpgrade, close_code};
use axum::http::HeaderMap;
use axum::response::Response;
use kokonoe_engine::RULES_VERSION;
use kokonoe_protocol::{ClientMsg, CloseCode, PROTOCOL};
use tokio::sync::mpsc;

use crate::room::{Command, RoomId};
use crate::rooms::{Joined, Refused};
use crate::session::{self, Session};
use crate::{AppState, Unavailable, from_here, not_from_here};

/// How long a socket has to say `Hello`.
const HELLO_TIMEOUT: Duration = Duration::from_secs(10);

/// How often the server pings an open socket, so proxies and phones don't drop it as idle while
/// a player thinks.
const KEEPALIVE: Duration = Duration::from_secs(30);

/// How many messages wait for a slow socket before its Room drops it.
const QUEUE: usize = 32;

/// The largest frame a client may send: a `Hello` or a `Move` is far smaller.
const MAX_MESSAGE: usize = 4096;

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
    Ok(upgrade.on_upgrade(move |socket| async move {
        match found {
            Some(session) => play(socket, state, session).await,
            None => close(socket, CloseCode::Forbidden.code(), "no session").await,
        }
    }))
}

async fn play(mut socket: WebSocket, state: AppState, session: Session) {
    let hello = match tokio::time::timeout(HELLO_TIMEOUT, first(&mut socket)).await {
        Ok(Some(Message::Text(text))) => serde_json::from_str::<ClientMsg>(&text),
        Ok(Some(Message::Binary(_))) => {
            return close(socket, close_code::UNSUPPORTED, "text only").await;
        }
        Ok(_) => return,
        Err(_) => return close(socket, close_code::POLICY, "no hello").await,
    };
    let room = match hello {
        Ok(ClientMsg::Hello {
            protocol,
            rules_version,
            room,
            ..
        }) => {
            if protocol != PROTOCOL || rules_version != RULES_VERSION {
                let code = CloseCode::UpdateRequired.code();
                return close(socket, code, "another build").await;
            }
            room
        }
        Ok(_) => return close(socket, close_code::POLICY, "hello first").await,
        Err(_) => return close(socket, close_code::INVALID, "not a message").await,
    };
    let Some(room) = RoomId::parse(&room) else {
        return close(socket, CloseCode::NoRoom.code(), "no such Room").await;
    };
    let (out, mut inbox) = mpsc::channel(QUEUE);
    let joined = state
        .rooms
        .join(&room, session.account, &session.handle, &out)
        .await;
    drop(out);
    let Joined { conn, room } = match joined {
        Ok(joined) => joined,
        Err(Refused::NoRoom) => {
            return close(socket, CloseCode::NoRoom.code(), "no such Room").await;
        }
        Err(Refused::Full) => return close(socket, CloseCode::Forbidden.code(), "full").await,
        Err(Refused::Unavailable) => {
            return close(socket, close_code::ERROR, "unavailable").await;
        }
    };

    let mut keepalive =
        tokio::time::interval_at(tokio::time::Instant::now() + KEEPALIVE, KEEPALIVE);
    loop {
        tokio::select! {
            received = socket.recv() => match received {
                Some(Ok(Message::Text(text))) => match serde_json::from_str(&text) {
                    Ok(ClientMsg::Move { ply, cell }) => {
                        if room.send(Command::Move { conn, ply, cell }).await.is_err() {
                            return close(socket, close_code::ERROR, "the Room stopped").await;
                        }
                    }
                    Ok(ClientMsg::Hello { .. }) => {
                        let code = CloseCode::Forbidden.code();
                        return close(socket, code, "hello again").await;
                    }
                    Err(_) => return close(socket, close_code::INVALID, "not a message").await,
                },
                Some(Ok(Message::Binary(_))) => {
                    return close(socket, close_code::UNSUPPORTED, "text only").await;
                }
                Some(Ok(Message::Close(_))) | Some(Err(_)) | None => return,
                Some(Ok(Message::Ping(_) | Message::Pong(_))) => {}
            },
            sent = inbox.recv() => match sent {
                Some(message) => {
                    let text = serde_json::to_string(&message).expect("a message is always JSON");
                    if socket.send(Message::Text(text.into())).await.is_err() {
                        return;
                    }
                }
                // The Room dropped this socket, or stopped: the page reconnects for a fresh
                // Welcome.
                None => return close(socket, close_code::ERROR, "reconnect").await,
            },
            _ = keepalive.tick() => {
                if socket.send(Message::Ping(Default::default())).await.is_err() {
                    return;
                }
            }
        }
    }
}

/// The first message that isn't a ping or a pong; nothing once the socket closes.
async fn first(socket: &mut WebSocket) -> Option<Message> {
    loop {
        match socket.recv().await?.ok()? {
            Message::Ping(_) | Message::Pong(_) => {}
            message => return Some(message),
        }
    }
}

async fn close(mut socket: WebSocket, code: u16, reason: &str) {
    let close = CloseFrame {
        code,
        reason: reason.into(),
    };
    let _ = socket.send(Message::Close(Some(close))).await;
}
