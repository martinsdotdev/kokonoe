//! Every Room: the routes that make one and describe one, and the registry that finds the task
//! playing one, loading it from Postgres the first time a socket asks for it after a start.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, PoisonError};

use axum::Json;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse, Response};
use kokonoe_engine::{Game, Player, Preset, Ruleset, Seed};
use kokonoe_protocol::{CreatedRoom, NewRoom, RoomCard, SeatChoice, ServerMsg};
use sqlx::PgPool;
use tokio::sync::{mpsc, oneshot};

use crate::random::bytes;
use crate::room::{self, Command, Conn, Full, RoomId};
use crate::session::{self, AccountId};
use crate::{AppState, Unavailable, games};

/// The Rooms being played, each reached through its task's commands.
#[derive(Clone)]
pub struct Rooms {
    pool: PgPool,
    live: Arc<Mutex<HashMap<RoomId, mpsc::Sender<Command>>>>,
}

/// A socket's place in a Room.
pub struct Joined {
    pub conn: Conn,
    pub room: mpsc::Sender<Command>,
}

/// Why a socket has no place in a Room.
#[derive(Debug)]
pub enum Refused {
    NoRoom,
    Full,
    /// The database didn't answer, or holds a game this build can't open.
    Unavailable,
}

impl Rooms {
    pub fn new(pool: PgPool) -> Rooms {
        Rooms {
            pool,
            live: Arc::default(),
        }
    }

    /// Seats `account` in the Room `id`: the seat it already holds, or an empty one. Its messages
    /// go to `out`. A Room whose task has stopped is loaded again, once.
    pub async fn join(
        &self,
        id: &RoomId,
        account: AccountId,
        handle: &str,
        out: &mpsc::Sender<ServerMsg>,
    ) -> Result<Joined, Refused> {
        for _ in 0..2 {
            let room = self.find(id).await?;
            let (reply, answer) = oneshot::channel();
            let join = Command::Join {
                account,
                handle: handle.to_owned(),
                out: out.clone(),
                reply,
            };
            if room.send(join).await.is_ok() {
                match answer.await {
                    Ok(Ok(conn)) => return Ok(Joined { conn, room }),
                    Ok(Err(Full)) => return Err(Refused::Full),
                    Err(_) => {}
                }
            }
            self.forget(id, &room);
        }
        Err(Refused::Unavailable)
    }

    /// The task playing the Room `id`, started from Postgres if none is.
    async fn find(&self, id: &RoomId) -> Result<mpsc::Sender<Command>, Refused> {
        if let Some(room) = self.running(id) {
            return Ok(room);
        }
        let stored = match games::load(&self.pool, id).await {
            Ok(Some(stored)) => stored,
            Ok(None) => return Err(Refused::NoRoom),
            Err(error) => {
                tracing::warn!(%error, room = %id, "a Room couldn't be loaded");
                return Err(Refused::Unavailable);
            }
        };
        let game = Game::open(&stored.record).map_err(|error| {
            tracing::error!(?error, room = %id, "a stored game doesn't replay");
            Refused::Unavailable
        })?;
        let mut live = self.live.lock().unwrap_or_else(PoisonError::into_inner);
        // Another socket may have loaded it meanwhile; the first task started is the one.
        if let Some(room) = live.get(id).filter(|room| !room.is_closed()) {
            return Ok(room.clone());
        }
        let room = room::spawn(id.clone(), self.pool.clone(), game, stored.seats);
        live.insert(id.clone(), room.clone());
        Ok(room)
    }

    fn running(&self, id: &RoomId) -> Option<mpsc::Sender<Command>> {
        let live = self.live.lock().unwrap_or_else(PoisonError::into_inner);
        live.get(id).filter(|room| !room.is_closed()).cloned()
    }

    /// Forgets the task `room` for `id`, which has stopped, unless another has replaced it.
    fn forget(&self, id: &RoomId, room: &mpsc::Sender<Command>) {
        let mut live = self.live.lock().unwrap_or_else(PoisonError::into_inner);
        if live.get(id).is_some_and(|known| known.same_channel(room)) {
            live.remove(id);
        }
    }
}

/// `POST /api/rooms`: makes a Room for the session's Account, playing a Preset, with the host in
/// the seat they chose or one drawn for them. Its link is `/r/<room>`.
pub async fn create(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(asked): Json<NewRoom>,
) -> Result<Response, Unavailable> {
    let Some(host) = session::find(&state.pool, &headers).await? else {
        return Ok((StatusCode::FORBIDDEN, "no session").into_response());
    };
    let preset = asked
        .rules
        .parse::<Ruleset>()
        .ok()
        .filter(|ruleset| Preset::of(ruleset).is_some());
    let Some(ruleset) = preset else {
        return Ok((StatusCode::UNPROCESSABLE_ENTITY, "not a Preset's code").into_response());
    };
    let seat = match asked.seat {
        SeatChoice::X => Player::X,
        SeatChoice::O => Player::O,
        SeatChoice::Random if bytes::<1>()[0] & 1 == 0 => Player::X,
        SeatChoice::Random => Player::O,
    };
    let room = RoomId::new();
    let seed = Seed(u64::from_le_bytes(bytes()));
    games::insert(&state.pool, &room, ruleset, seed, host.account, seat).await?;
    let link = [(header::LOCATION, format!("/r/{room}"))];
    let created = CreatedRoom {
        room: room.to_string(),
    };
    Ok((StatusCode::CREATED, link, Json(created)).into_response())
}

/// `GET /api/rooms/{room}`: what the link's holder is asked to join, read from Postgres, which
/// holds every seat before anyone is told of it.
pub async fn card(
    State(state): State<AppState>,
    Path(room): Path<String>,
    headers: HeaderMap,
) -> Result<Response, Unavailable> {
    let never = [(header::CACHE_CONTROL, "no-store")];
    let summary = match RoomId::parse(&room) {
        Some(room) => games::summary(&state.pool, &room).await?,
        None => None,
    };
    let Some(summary) = summary else {
        return Ok((StatusCode::NOT_FOUND, never, "no such Room").into_response());
    };
    let asker = session::find(&state.pool, &headers).await?;
    let card = RoomCard {
        variant: summary.variant().to_owned(),
        you: asker.and_then(|asker| summary.seats.of(asker.account)),
        players: summary.seats.players(),
        host: summary.host,
        rules: summary.ruleset_code,
    };
    Ok((never, Json(card)).into_response())
}
