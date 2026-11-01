//! Invite Rooms as the tests play them: a Postgres, the app for its routes, a server for its
//! sockets, and a Room with both seats filled.

use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use http_body_util::BodyExt;
use kokonoe_engine::{Cell, Player, RULES_VERSION, Record};
use kokonoe_protocol::{ClientMsg, PROTOCOL, Players, Rejection, Role, ServerMsg};
use sqlx::PgPool;
use tempfile::TempDir;
use tower::ServiceExt;

use super::{
    Container, ORIGIN, Server, Socket, cookie, database, guest, hear, open, say, session_id,
};

/// X takes Local boards 0, 1 and 2, each with a row, while O sends X wherever it needs to go:
/// `TOP_ROW` in `web/src/lib/game/games.fixtures.ts`.
pub const TOP_ROW: &str = "03 30 04 40 05 51 13 31 14 41 15 52 26 62 27 72 28";

/// The opening of `LINE_OF_MIDDLE_BOARDS`, the engine's Deep game in `tests/deep.rs`.
pub const DEEP_OPENING: &str = "280 803 031";

pub fn cells(digits: &str) -> Vec<u16> {
    digits
        .split(' ')
        .map(|cell| {
            let digits: Vec<u8> = cell.bytes().map(|digit| digit - b'0').collect();
            Cell::from_digits(&digits).unwrap().index()
        })
        .collect()
}

/// Everything a test needs: Postgres, the app for its HTTP routes, and a server for sockets.
pub struct World {
    pub _container: Container,
    pub url: String,
    pub pool: PgPool,
    pub app: Router,
    pub _root: TempDir,
    pub server: Server,
}

pub async fn world() -> World {
    let (container, url) = database().await;
    let pool = kokonoe_server::connect(&url).await.unwrap();
    let (app, root) = super::app(pool.clone());
    let server = Server::start(&url).await;
    World {
        _container: container,
        url,
        pool,
        app,
        _root: root,
        server,
    }
}

/// A Room with both seats filled and both sockets welcomed, the host as X.
pub struct Table {
    pub room: String,
    pub host: String,
    pub joiner: String,
    pub x: Socket,
    pub o: Socket,
    /// Who sits where, as the last `Welcome` said.
    pub players: Players,
}

impl World {
    /// A Room of `rules` with both seats filled, the host as X.
    pub async fn table(&self, rules: &str) -> Table {
        self.table_with(&format!(r#"{{"rules":"{rules}","seat":"X"}}"#))
            .await
    }

    /// A Room made with `body`, `POST /api/rooms`'s JSON, with both seats filled, the host first.
    pub async fn table_with(&self, body: &str) -> Table {
        let (host, joiner) = (guest(&self.app).await, guest(&self.app).await);
        let made = create(&self.app, Some(&host), body).await;
        assert_eq!(made.status, StatusCode::CREATED, "{body}");
        let room = made.json["room"].as_str().unwrap().to_owned();
        let mut x = self.enter(&host, &room).await;
        welcomed(&mut x).await;
        let mut o = self.enter(&joiner, &room).await;
        welcomed(&mut x).await;
        let (_, _, players) = welcomed(&mut o).await;
        Table {
            room,
            host,
            joiner,
            x,
            o,
            players,
        }
    }

    /// A socket that has said `Hello` to `room`.
    pub async fn enter(&self, token: &str, room: &str) -> Socket {
        let mut socket = open(self.server.address, token).await;
        say(&mut socket, &hello(room)).await;
        socket
    }
}

pub struct Answer {
    pub status: StatusCode,
    pub location: Option<String>,
    pub cache: Option<String>,
    pub json: serde_json::Value,
}

pub async fn fetch(app: &Router, request: Request<Body>) -> Answer {
    let response = app.clone().oneshot(request).await.unwrap();
    let header = |name| {
        response
            .headers()
            .get(name)
            .map(|value: &axum::http::HeaderValue| value.to_str().unwrap().to_owned())
    };
    let (status, location, cache) = (
        response.status(),
        header(header::LOCATION),
        header(header::CACHE_CONTROL),
    );
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let json = serde_json::from_slice(&body).unwrap_or(serde_json::Value::Null);
    Answer {
        status,
        location,
        cache,
        json,
    }
}

/// `POST /api/rooms` with `body`, holding `token` if any.
pub async fn create(app: &Router, token: Option<&str>, body: &str) -> Answer {
    let mut request = Request::post("/api/rooms")
        .header(header::ORIGIN, ORIGIN)
        .header(header::CONTENT_TYPE, "application/json");
    if let Some(token) = token {
        request = request.header(header::COOKIE, cookie(token));
    }
    fetch(app, request.body(Body::from(body.to_owned())).unwrap()).await
}

/// A Room of `rules` with `token`'s Guest in `seat`.
pub async fn room(app: &Router, token: &str, rules: &str, seat: &str) -> String {
    let body = format!(r#"{{"rules":"{rules}","seat":"{seat}"}}"#);
    let made = create(app, Some(token), &body).await;
    assert_eq!(made.status, StatusCode::CREATED);
    made.json["room"].as_str().unwrap().to_owned()
}

/// `GET /api/rooms/{room}`, holding `token` if any.
pub async fn card(app: &Router, token: Option<&str>, room: &str) -> Answer {
    let mut request = Request::get(format!("/api/rooms/{room}"));
    if let Some(token) = token {
        request = request.header(header::COOKIE, cookie(token));
    }
    fetch(app, request.body(Body::empty()).unwrap()).await
}

/// The page at `path`.
pub async fn page(app: &Router, path: &str) -> String {
    let request = Request::get(path).body(Body::empty()).unwrap();
    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    String::from_utf8(body.to_vec()).unwrap()
}

pub async fn handle(pool: &PgPool, token: &str) -> String {
    sqlx::query_scalar(
        "SELECT a.handle::text FROM sessions s JOIN accounts a ON a.id = s.account_id
         WHERE s.id = $1",
    )
    .bind(session_id(token))
    .fetch_one(pool)
    .await
    .unwrap()
}

pub async fn stored(pool: &PgPool, room: &str) -> Vec<i32> {
    sqlx::query_scalar("SELECT moves FROM games WHERE room = $1")
        .bind(room)
        .fetch_one(pool)
        .await
        .unwrap()
}

pub fn hello(room: &str) -> ClientMsg {
    ClientMsg::Hello {
        protocol: PROTOCOL,
        rules_version: RULES_VERSION,
        room: room.to_owned(),
        role: Role::Player,
    }
}

pub async fn step(socket: &mut Socket, ply: u16, cell: u16) {
    say(socket, &ClientMsg::Move { ply, cell }).await;
}

/// The `Welcome` a socket hears next: its seat, the Record and the players.
pub async fn welcomed(socket: &mut Socket) -> (Player, Record, Players) {
    match hear(socket).await {
        ServerMsg::Welcome { you, snapshot } => (you, snapshot.record, snapshot.players),
        other => panic!("not a Welcome: {other:?}"),
    }
}

pub fn indices(record: &Record) -> Vec<u16> {
    record.moves.iter().map(|cell| cell.index()).collect()
}

pub fn moved(ply: u16, cell: u16) -> ServerMsg {
    ServerMsg::Moved { ply, cell }
}

pub fn rejected(reason: Rejection) -> ServerMsg {
    ServerMsg::Rejected { reason }
}

/// Plays `cells` from `ply`, each by the side to move, and checks both sockets hear each move.
pub async fn play(table: &mut Table, ply: u16, cells: &[u16]) {
    for (ply, &cell) in (ply..).zip(cells) {
        let mover = if ply % 2 == 0 {
            &mut table.x
        } else {
            &mut table.o
        };
        step(mover, ply, cell).await;
        assert_eq!(hear(&mut table.x).await, moved(ply, cell));
        assert_eq!(hear(&mut table.o).await, moved(ply, cell));
    }
}

/// Sends each move by the side named, and checks it's refused for its reason.
pub async fn refuse(table: &mut Table, moves: &[(Player, u16, u16, Rejection)]) {
    for &(by, ply, cell, reason) in moves {
        let socket = match by {
            Player::X => &mut table.x,
            Player::O => &mut table.o,
        };
        step(socket, ply, cell).await;
        assert_eq!(
            hear(socket).await,
            rejected(reason),
            "ply {ply}, Cell {cell}"
        );
    }
}
