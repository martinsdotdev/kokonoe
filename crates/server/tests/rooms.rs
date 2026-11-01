//! Invite Rooms (spec §10) against a real Postgres in Docker: making one, describing it to a
//! joiner, and playing it over two sockets on a server that can be killed and started again.

mod common;

use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use common::{ORIGIN, Server, Socket, closed, database, guest, hear, open, say};
use http_body_util::BodyExt;
use kokonoe_engine::{Cell, Outcome, Player, Preset, RULES_VERSION, Record};
use kokonoe_protocol::{ClientMsg, PROTOCOL, Players, Rejection, Role, ServerMsg};
use sqlx::PgPool;
use tempfile::TempDir;
use tower::ServiceExt;

/// X takes Local boards 0, 1 and 2, each with a row, while O sends X wherever it needs to go:
/// `TOP_ROW` in `web/src/lib/game/games.fixtures.ts`.
const TOP_ROW: &str = "03 30 04 40 05 51 13 31 14 41 15 52 26 62 27 72 28";

fn top_row() -> Vec<u16> {
    TOP_ROW
        .split(' ')
        .map(|digits| {
            let digits: Vec<u8> = digits.bytes().map(|digit| digit - b'0').collect();
            Cell::from_digits(&digits).unwrap().index()
        })
        .collect()
}

/// Everything a test needs: Postgres, the app for its HTTP routes, and a server for sockets.
struct World {
    _container: testcontainers_modules::testcontainers::ContainerAsync<
        testcontainers_modules::postgres::Postgres,
    >,
    url: String,
    pool: PgPool,
    app: Router,
    _root: TempDir,
    server: Server,
}

async fn world() -> World {
    let (container, url) = database().await;
    let pool = kokonoe_server::connect(&url).await.unwrap();
    let (app, root) = common::app(pool.clone());
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

struct Answer {
    status: StatusCode,
    location: Option<String>,
    cache: Option<String>,
    json: serde_json::Value,
}

async fn fetch(app: &Router, request: Request<Body>) -> Answer {
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
async fn create(app: &Router, token: Option<&str>, body: &str) -> Answer {
    let mut request = Request::post("/api/rooms")
        .header(header::ORIGIN, ORIGIN)
        .header(header::CONTENT_TYPE, "application/json");
    if let Some(token) = token {
        request = request.header(header::COOKIE, format!("__Host-session={token}"));
    }
    fetch(app, request.body(Body::from(body.to_owned())).unwrap()).await
}

/// A Standard Room with `token`'s Guest in `seat`.
async fn room(app: &Router, token: &str, seat: &str) -> String {
    let made = create(
        app,
        Some(token),
        &format!(r#"{{"rules":"std","seat":"{seat}"}}"#),
    )
    .await;
    assert_eq!(made.status, StatusCode::CREATED);
    made.json["room"].as_str().unwrap().to_owned()
}

/// `GET /api/rooms/{room}`, holding `token` if any.
async fn card(app: &Router, token: Option<&str>, room: &str) -> Answer {
    let mut request = Request::get(format!("/api/rooms/{room}"));
    if let Some(token) = token {
        request = request.header(header::COOKIE, format!("__Host-session={token}"));
    }
    fetch(app, request.body(Body::empty()).unwrap()).await
}

async fn handle(pool: &PgPool, token: &str) -> String {
    let (id, _) = token.split_once('.').unwrap();
    sqlx::query_scalar(
        "SELECT a.handle::text FROM sessions s JOIN accounts a ON a.id = s.account_id
         WHERE s.id = $1",
    )
    .bind(id)
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn stored(pool: &PgPool, room: &str) -> Vec<i32> {
    sqlx::query_scalar("SELECT moves FROM games WHERE room = $1")
        .bind(room)
        .fetch_one(pool)
        .await
        .unwrap()
}

fn hello(room: &str) -> ClientMsg {
    ClientMsg::Hello {
        protocol: PROTOCOL,
        rules_version: RULES_VERSION,
        room: room.to_owned(),
        role: Role::Player,
    }
}

/// A socket that has said `Hello` to `room`.
async fn enter(server: &Server, token: &str, room: &str) -> Socket {
    let mut socket = open(server.address, token).await;
    say(&mut socket, &hello(room)).await;
    socket
}

/// The `Welcome` a socket hears next: its seat, the Record and the players.
async fn welcomed(socket: &mut Socket) -> (Player, Record, Players) {
    match hear(socket).await {
        ServerMsg::Welcome { you, snapshot } => (you, snapshot.record, snapshot.players),
        other => panic!("not a Welcome: {other:?}"),
    }
}

/// A Room with both seats filled, the host as X: its id, the host's token, and X's and O's
/// sockets.
async fn seated(world: &World) -> (String, String, Socket, Socket) {
    let (host, joiner) = (guest(&world.app).await, guest(&world.app).await);
    let room = room(&world.app, &host, "X").await;
    let mut x = enter(&world.server, &host, &room).await;
    welcomed(&mut x).await;
    let mut o = enter(&world.server, &joiner, &room).await;
    welcomed(&mut x).await;
    welcomed(&mut o).await;
    (room, host, x, o)
}

fn moved(ply: u16, cell: u16) -> ServerMsg {
    ServerMsg::Moved { ply, cell }
}

fn rejected(reason: Rejection) -> ServerMsg {
    ServerMsg::Rejected { reason }
}

/// Plays `cells` from `ply`, each by the side to move, and checks both sockets hear each move.
async fn play(x: &mut Socket, o: &mut Socket, ply: u16, cells: &[u16]) {
    for (ply, &cell) in (ply..).zip(cells) {
        let mover = if ply % 2 == 0 { &mut *x } else { &mut *o };
        say(mover, &ClientMsg::Move { ply, cell }).await;
        assert_eq!(hear(x).await, moved(ply, cell));
        assert_eq!(hear(o).await, moved(ply, cell));
    }
}

/// Sends each move by the side named, and checks it's refused for its reason.
async fn refuse(x: &mut Socket, o: &mut Socket, moves: &[(Player, u16, u16, Rejection)]) {
    for &(by, ply, cell, reason) in moves {
        let socket = match by {
            Player::X => &mut *x,
            Player::O => &mut *o,
        };
        say(socket, &ClientMsg::Move { ply, cell }).await;
        assert_eq!(
            hear(socket).await,
            rejected(reason),
            "ply {ply}, Cell {cell}"
        );
    }
}

#[tokio::test]
async fn creating_a_room_needs_a_session_and_a_preset() {
    let world = world().await;
    let standard = r#"{"rules":"std","seat":"X"}"#;
    assert_eq!(
        create(&world.app, None, standard).await.status,
        StatusCode::FORBIDDEN
    );
    let host = guest(&world.app).await;
    for rules in ["standard", "cont+both"] {
        let body = format!(r#"{{"rules":"{rules}","seat":"X"}}"#);
        let refused = create(&world.app, Some(&host), &body).await;
        assert_eq!(refused.status, StatusCode::UNPROCESSABLE_ENTITY, "{rules}");
    }

    let made = create(&world.app, Some(&host), standard).await;
    assert_eq!(made.status, StatusCode::CREATED);
    let room = made.json["room"].as_str().unwrap();
    assert_eq!(room.len(), 20);
    assert!(
        room.bytes()
            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
    );
    assert_eq!(made.location.as_deref(), Some(&*format!("/r/{room}")));

    let (host_is_x, o_empty, moves, seed, version, code, ruleset): (
        bool,
        bool,
        Vec<i32>,
        String,
        i32,
        String,
        String,
    ) = sqlx::query_as(
        "SELECT x_id = host_id, o_id IS NULL, moves, seed, rules_version, ruleset_code,
                ruleset::text
         FROM games WHERE room = $1",
    )
    .bind(room)
    .fetch_one(&world.pool)
    .await
    .unwrap();
    assert!(host_is_x && o_empty);
    assert_eq!(moves, Vec::<i32>::new());
    assert_eq!(seed.len(), 16);
    assert_eq!((version, &*code), (RULES_VERSION as i32, "std"));
    let ruleset: serde_json::Value = serde_json::from_str(&ruleset).unwrap();
    assert_eq!(
        ruleset,
        serde_json::to_value(Preset::Standard.ruleset()).unwrap()
    );
}

#[tokio::test]
async fn a_random_seat_lands_on_either_side() {
    let world = world().await;
    let host = guest(&world.app).await;
    for _ in 0..64 {
        room(&world.app, &host, "Random").await;
    }
    let (x, o): (i64, i64) = sqlx::query_as(
        "SELECT count(*) FILTER (WHERE x_id IS NOT NULL), count(*) FILTER (WHERE o_id IS NOT NULL)
         FROM games",
    )
    .fetch_one(&world.pool)
    .await
    .unwrap();
    assert_eq!(x + o, 64);
    assert!(x > 0 && o > 0, "{x} as X, {o} as O");
}

#[tokio::test]
async fn the_card_tells_a_joiner_what_they_join() {
    let world = world().await;
    let host = guest(&world.app).await;
    let room = room(&world.app, &host, "O").await;
    let name = handle(&world.pool, &host).await;

    let stranger = card(&world.app, None, &room).await;
    assert_eq!(stranger.status, StatusCode::OK);
    assert_eq!(stranger.cache.as_deref(), Some("no-store"));
    assert_eq!(
        stranger.json,
        serde_json::json!({
            "host": name,
            "variant": "Standard",
            "rules": "std",
            "players": { "X": null, "O": name },
            "you": null,
        })
    );
    let own = card(&world.app, Some(&host), &room).await;
    assert_eq!(own.json["you"], "O");

    for missing in ["00000000000000000000", "abc", "6F1C0A9B3E2D4C5A7B8E"] {
        let answer = card(&world.app, None, missing).await;
        assert_eq!(answer.status, StatusCode::NOT_FOUND, "{missing}");
        assert_eq!(answer.cache.as_deref(), Some("no-store"), "{missing}");
    }
}

#[tokio::test]
async fn two_players_play_a_whole_game() {
    let world = world().await;
    let (host, joiner) = (guest(&world.app).await, guest(&world.app).await);
    let room = room(&world.app, &host, "X").await;
    let (host_name, joiner_name) = (
        handle(&world.pool, &host).await,
        handle(&world.pool, &joiner).await,
    );

    let mut x = enter(&world.server, &host, &room).await;
    let (you, record, players) = welcomed(&mut x).await;
    assert_eq!(you, Player::X);
    assert_eq!(record.ruleset, Preset::Standard.ruleset());
    assert!(record.moves.is_empty());
    assert_eq!(players.get(Player::O), None, "waiting for the joiner");

    let mut o = enter(&world.server, &joiner, &room).await;
    for (socket, seat) in [(&mut x, Player::X), (&mut o, Player::O)] {
        let (you, _, players) = welcomed(socket).await;
        assert_eq!(you, seat);
        assert_eq!(players.get(Player::X), Some(&*host_name));
        assert_eq!(players.get(Player::O), Some(&*joiner_name));
    }

    let cells = top_row();
    let (last, rest) = cells.split_last().unwrap();
    for (ply, &cell) in (0..).zip(rest) {
        play(&mut x, &mut o, ply, &[cell]).await;
        let written = stored(&world.pool, &room).await;
        assert_eq!(
            written.len(),
            usize::from(ply) + 1,
            "stored before it was told"
        );
    }
    let ply = rest.len() as u16;
    say(&mut x, &ClientMsg::Move { ply, cell: *last }).await;
    let ended = ServerMsg::Ended {
        outcome: Outcome::Objective(Player::X),
    };
    for socket in [&mut x, &mut o] {
        assert_eq!(hear(socket).await, moved(ply, *last));
        assert_eq!(hear(socket).await, ended);
    }
    let result: Option<String> = sqlx::query_scalar("SELECT result FROM games WHERE room = $1")
        .bind(&room)
        .fetch_one(&world.pool)
        .await
        .unwrap();
    assert_eq!(result.as_deref(), Some("1-0"));

    say(
        &mut o,
        &ClientMsg::Move {
            ply: ply + 1,
            cell: 0,
        },
    )
    .await;
    assert_eq!(hear(&mut o).await, ended, "a move after the end");
}

#[tokio::test]
async fn a_refused_move_changes_nothing() {
    let world = world().await;
    let (host, joiner) = (guest(&world.app).await, guest(&world.app).await);
    let room = room(&world.app, &host, "X").await;
    let mut x = enter(&world.server, &host, &room).await;
    welcomed(&mut x).await;
    say(&mut x, &ClientMsg::Move { ply: 0, cell: 40 }).await;
    assert_eq!(
        hear(&mut x).await,
        rejected(Rejection::NotAllowed),
        "before the joiner"
    );

    let mut o = enter(&world.server, &joiner, &room).await;
    welcomed(&mut x).await;
    welcomed(&mut o).await;
    let refusals = [
        (Player::X, 0, 81, Rejection::IllegalMove),
        (Player::O, 0, 40, Rejection::NotAllowed),
        (Player::X, 2, 40, Rejection::OutOfOrder { want: 0 }),
    ];
    refuse(&mut x, &mut o, &refusals).await;
    play(&mut x, &mut o, 0, &[40]).await;
    let refusals = [
        (Player::X, 1, 36, Rejection::NotAllowed),
        (Player::O, 1, 40, Rejection::IllegalMove),
        (Player::O, 1, 0, Rejection::IllegalMove),
    ];
    refuse(&mut x, &mut o, &refusals).await;

    assert_eq!(stored(&world.pool, &room).await, [40]);
    let mut again = enter(&world.server, &host, &room).await;
    let (_, record, _) = welcomed(&mut again).await;
    assert_eq!(
        record
            .moves
            .iter()
            .map(|cell| cell.index())
            .collect::<Vec<_>>(),
        [40]
    );
}

#[tokio::test]
async fn a_resent_move_says_nothing_new() {
    let world = world().await;
    let (room, _, mut x, mut o) = seated(&world).await;
    play(&mut x, &mut o, 0, &[40]).await;
    say(&mut x, &ClientMsg::Move { ply: 0, cell: 40 }).await;
    play(&mut x, &mut o, 1, &[36]).await;
    assert_eq!(stored(&world.pool, &room).await, [40, 36]);
}

#[tokio::test]
async fn hello_checks_the_build_and_the_room() {
    let world = world().await;
    let (room, _, _x, _o) = seated(&world).await;
    let stranger = guest(&world.app).await;

    let builds = [(PROTOCOL + 1, RULES_VERSION), (PROTOCOL, RULES_VERSION + 1)];
    for (protocol, rules_version) in builds {
        let mut socket = open(world.server.address, &stranger).await;
        let hello = ClientMsg::Hello {
            protocol,
            rules_version,
            room: room.clone(),
            role: Role::Player,
        };
        say(&mut socket, &hello).await;
        assert_eq!(
            closed(&mut socket).await,
            4001,
            "{protocol}, {rules_version}"
        );
    }
    for missing in ["00000000000000000000", "abc"] {
        let mut socket = enter(&world.server, &stranger, missing).await;
        assert_eq!(closed(&mut socket).await, 4004, "{missing}");
    }
    let mut third = enter(&world.server, &stranger, &room).await;
    assert_eq!(closed(&mut third).await, 4003, "full");

    let host = guest(&world.app).await;
    let other = self::room(&world.app, &host, "X").await;
    let mut twice = enter(&world.server, &host, &other).await;
    welcomed(&mut twice).await;
    say(&mut twice, &hello(&other)).await;
    assert_eq!(closed(&mut twice).await, 4003, "a second Hello");

    let mut first = open(world.server.address, &host).await;
    say(&mut first, &ClientMsg::Move { ply: 0, cell: 40 }).await;
    assert_eq!(closed(&mut first).await, 1008, "a move before Hello");
}

#[tokio::test]
async fn a_reconnect_gets_a_fresh_welcome_and_plays_on() {
    let world = world().await;
    let (host, joiner) = (guest(&world.app).await, guest(&world.app).await);
    let room = room(&world.app, &host, "X").await;
    let mut x = enter(&world.server, &host, &room).await;
    welcomed(&mut x).await;
    let mut o = enter(&world.server, &joiner, &room).await;
    welcomed(&mut x).await;
    welcomed(&mut o).await;
    let cells = top_row();
    play(&mut x, &mut o, 0, &cells[..3]).await;

    drop(o);
    let mut o = enter(&world.server, &joiner, &room).await;
    let (you, record, players) = welcomed(&mut o).await;
    assert_eq!(you, Player::O);
    assert_eq!(record.moves.len(), 3);
    assert!(players.get(Player::X).is_some() && players.get(Player::O).is_some());
    play(&mut x, &mut o, 3, &cells[3..5]).await;
}

#[tokio::test]
async fn a_restart_rebuilds_every_room_from_postgres() {
    let mut world = world().await;
    let (host, joiner) = (guest(&world.app).await, guest(&world.app).await);
    let room = room(&world.app, &host, "X").await;
    let mut x = enter(&world.server, &host, &room).await;
    welcomed(&mut x).await;
    let mut o = enter(&world.server, &joiner, &room).await;
    welcomed(&mut x).await;
    let (_, _, players) = welcomed(&mut o).await;
    let cells = top_row();
    play(&mut x, &mut o, 0, &cells[..8]).await;

    let server = std::mem::replace(&mut world.server, Server::start(&world.url).await);
    server.kill();
    drop((x, o));

    let mut x = enter(&world.server, &host, &room).await;
    let mut o = enter(&world.server, &joiner, &room).await;
    for (socket, seat) in [(&mut x, Player::X), (&mut o, Player::O)] {
        let (you, record, again) = welcomed(socket).await;
        assert_eq!(you, seat);
        assert_eq!(again, players);
        let moves: Vec<u16> = record.moves.iter().map(|cell| cell.index()).collect();
        assert_eq!(moves, cells[..8]);
    }
    let (last, rest) = cells.split_last().unwrap();
    play(&mut x, &mut o, 8, &rest[8..]).await;
    let ply = rest.len() as u16;
    say(&mut x, &ClientMsg::Move { ply, cell: *last }).await;
    assert_eq!(hear(&mut o).await, moved(ply, *last));
    assert!(matches!(hear(&mut o).await, ServerMsg::Ended { .. }));
}

#[tokio::test]
async fn a_move_that_cannot_be_written_is_never_acknowledged() {
    let world = world().await;
    let (room, host, mut x, mut o) = seated(&world).await;
    let cells = top_row();
    play(&mut x, &mut o, 0, &cells[..2]).await;

    sqlx::query("ALTER TABLE games ADD CONSTRAINT short CHECK (cardinality(moves) < 3)")
        .execute(&world.pool)
        .await
        .unwrap();
    say(
        &mut x,
        &ClientMsg::Move {
            ply: 2,
            cell: cells[2],
        },
    )
    .await;
    assert_eq!(closed(&mut x).await, 1011);
    assert_eq!(closed(&mut o).await, 1011);
    assert_eq!(stored(&world.pool, &room).await.len(), 2);

    sqlx::query("ALTER TABLE games DROP CONSTRAINT short")
        .execute(&world.pool)
        .await
        .unwrap();
    let mut x = enter(&world.server, &host, &room).await;
    let (_, record, _) = welcomed(&mut x).await;
    assert_eq!(record.moves.len(), 2, "rebuilt from what was stored");
    say(
        &mut x,
        &ClientMsg::Move {
            ply: 2,
            cell: cells[2],
        },
    )
    .await;
    assert_eq!(hear(&mut x).await, moved(2, cells[2]));
}
