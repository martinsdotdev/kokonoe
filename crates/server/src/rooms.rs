//! Every Room: the routes that make one and describe one, and the registry that finds the task
//! playing one, loading it from Postgres when no task is.

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
use crate::room::{self, Command, Conn, Join, RoomId, Timing};
use crate::session::{self, AccountId};
use crate::{AppState, Unavailable, games};

/// How many times a socket asks for its seat when the Room's task stops under it.
const ATTEMPTS: usize = 3;

/// The task playing a Room, if one has started. Its lock is held while the Room loads, so two
/// sockets never start two tasks, and none starts from what Postgres held before an earlier task
/// stopped.
type Slot = Arc<tokio::sync::Mutex<Option<mpsc::Sender<Command>>>>;

/// The Rooms being played. A Room's slot stays once it has loaded, a few bytes, while its task
/// stops when its last socket leaves.
#[derive(Clone)]
pub struct Rooms {
    pool: PgPool,
    timing: Timing,
    slots: Arc<Mutex<HashMap<RoomId, Slot>>>,
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
    pub fn new(pool: PgPool, timing: Timing) -> Rooms {
        Rooms {
            pool,
            timing,
            slots: Arc::default(),
        }
    }

    /// Seats `account` in the Room `id`: the seat it already holds, or an empty one. Its messages
    /// go to `out`. A Room whose task stops before answering is loaded again.
    pub async fn join(
        &self,
        id: &RoomId,
        account: AccountId,
        handle: &str,
        out: mpsc::Sender<ServerMsg>,
    ) -> Result<Joined, Refused> {
        for _ in 0..ATTEMPTS {
            let room = self.find(id).await?;
            let (reply, answer) = oneshot::channel();
            let join = Join {
                account,
                handle: handle.to_owned(),
                out: out.clone(),
                reply,
            };
            if room.send(Command::Join(join)).await.is_err() {
                continue;
            }
            match answer.await {
                Ok(Some(conn)) => return Ok(Joined { conn, room }),
                Ok(None) => return Err(Refused::Full),
                Err(_) => continue,
            }
        }
        Err(Refused::Unavailable)
    }

    /// The task playing the Room `id`, started from Postgres if none is.
    async fn find(&self, id: &RoomId) -> Result<mpsc::Sender<Command>, Refused> {
        let slot = {
            let mut slots = self.slots.lock().unwrap_or_else(PoisonError::into_inner);
            slots.entry(id.clone()).or_default().clone()
        };
        let mut task = slot.lock().await;
        if let Some(room) = task.as_ref().filter(|room| !room.is_closed()) {
            return Ok(room.clone());
        }
        let stored = match games::load(&self.pool, id).await {
            Ok(Some(stored)) => stored,
            Ok(None) => {
                // No slot for a Room that doesn't exist, so made-up ids can't fill the map.
                let mut slots = self.slots.lock().unwrap_or_else(PoisonError::into_inner);
                if slots.get(id).is_some_and(|known| Arc::ptr_eq(known, &slot)) {
                    slots.remove(id);
                }
                return Err(Refused::NoRoom);
            }
            Err(error) => {
                tracing::warn!(%error, room = %id, "a Room couldn't be loaded");
                return Err(Refused::Unavailable);
            }
        };
        let game = Game::open(&stored.record).map_err(|error| {
            tracing::error!(?error, room = %id, "a stored game doesn't replay");
            Refused::Unavailable
        })?;
        let room = room::spawn(id.clone(), self.pool.clone(), self.timing, game, stored);
        *task = Some(room.clone());
        Ok(room)
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
    let game = games::NewGame {
        ruleset,
        seed,
        host: host.account,
        seat,
        takebacks: asked.takebacks,
    };
    games::insert(&state.pool, &room, game).await?;
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
    let no_store = [(header::CACHE_CONTROL, "no-store")];
    let stored = match RoomId::parse(&room) {
        Some(room) => games::load(&state.pool, &room).await?,
        None => None,
    };
    let Some(stored) = stored else {
        return Ok((StatusCode::NOT_FOUND, no_store, "no such Room").into_response());
    };
    let asker = session::find(&state.pool, &headers).await?;
    let card = RoomCard {
        variant: stored.variant().to_owned(),
        rules: stored.record.ruleset.to_string(),
        you: asker.and_then(|asker| stored.seats.of(asker.account)),
        takebacks: stored.takebacks,
        players: stored.seats.players(),
        host: stored.host,
    };
    Ok((no_store, Json(card)).into_response())
}
