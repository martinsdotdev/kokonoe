//! A timed Room's clocks (spec §11), on `common::TIMING`'s short time: 3+2 is a 600 ms bank and
//! an 80 ms increment, 15+5 is 3 s and 200 ms, and a first move has 500 ms. The clocks'
//! arithmetic is `clock.rs`'s, on simulated time; these hold the Room to it over real sockets
//! and Postgres. A test that restarts the server plays 15+5, so a slow restart can't flag it.

mod common;

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use axum::http::StatusCode;
use common::table::*;
use common::{Server, Socket, closed, hear, say};
use kokonoe_engine::Player;
use kokonoe_protocol::{
    Abort, ClientMsg, Clocks, CloseCode, GameEnd, GameResult, OfferKind, ServerMsg, Snapshot,
    Termination, TimeControl,
};
use sqlx::PgPool;
use tokio::time::{Instant, sleep};

const BANK: u32 = 600;
const INCREMENT: u32 = 80;
const LONG_BANK: u32 = 3_000;
const LONG_INCREMENT: u32 = 200;
const ABORT: u32 = 500;

/// How far a clock may read below what the test slept for: the sockets' and the table's time.
const SLACK: u32 = 150;

fn ms(ms: u32) -> Duration {
    Duration::from_millis(u64::from(ms))
}

/// A Room under `control`, allowing takebacks, both seats filled, the host as X.
async fn timed(world: &World, control: TimeControl) -> Table {
    let body = format!(r#"{{"rules":"std","seat":"X","clock":"{control}","takebacks":true}}"#);
    world.table_with(&body).await
}

fn now_ms() -> u64 {
    let since = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
    u64::try_from(since.as_millis()).unwrap()
}

/// `cell` played at `ply` by the side to move, and the clocks both sockets heard with it.
async fn timed_step(table: &mut Table, ply: u16, cell: u16) -> Clocks {
    let mover = table.socket(kokonoe_engine::Ply::new(ply).player());
    step(mover, ply, cell).await;
    let mut heard = Vec::new();
    for socket in [&mut table.x, &mut table.o] {
        match hear(socket).await {
            ServerMsg::Moved {
                ply: at,
                cell: played,
                clocks: Some(clocks),
            } if (at, played) == (ply, cell) => heard.push(clocks),
            other => panic!("not ply {ply}'s move with its clocks: {other:?}"),
        }
    }
    assert_eq!(heard[0], heard[1], "both sockets hear the same clocks");
    heard[0]
}

/// Both sockets hear `message` next.
async fn both_hear(table: &mut Table, message: &ServerMsg) {
    for socket in [&mut table.x, &mut table.o] {
        assert_eq!(&hear(socket).await, message);
    }
}

fn ended(result: GameResult, termination: Termination) -> ServerMsg {
    ServerMsg::Ended(GameEnd {
        result,
        termination,
    })
}

const ABORTED: ServerMsg = ServerMsg::Aborted {
    reason: Abort::FirstMoveTimeout,
};

async fn games(pool: &PgPool, room: &str) -> i64 {
    sqlx::query_scalar("SELECT count(*) FROM games WHERE room = $1")
        .bind(room)
        .fetch_one(pool)
        .await
        .unwrap()
}

/// A fresh `Welcome` to `token`'s new socket in `room`, and its clocks.
async fn rejoined(world: &World, token: &str, room: &str) -> (Socket, Snapshot, Clocks) {
    let mut socket = world.enter(token, room).await;
    let (_, snapshot) = welcome(&mut socket).await;
    let clocks = snapshot.clocks.expect("a timed Room has clocks");
    (socket, snapshot, clocks)
}

/// Stops the server as a crash would, waits `down`, and starts it again on the same database.
async fn restart(world: &mut World, down: Duration) {
    world.server.kill();
    sleep(down).await;
    world.server = Server::start(&world.url).await;
}

#[tokio::test]
async fn the_first_moves_start_the_clocks_and_cost_nothing() {
    let world = world().await;
    let mut table = timed(&world, TimeControl::ThreePlusTwo).await;
    let moves = cells(TOP_ROW);

    let (_, snapshot, waiting) = rejoined(&world, &table.host, &table.room).await;
    assert_eq!(snapshot.clock, Some(TimeControl::ThreePlusTwo));
    assert_eq!(
        (waiting.x_ms, waiting.o_ms, waiting.running),
        (BANK, BANK, None)
    );
    let window = waiting.aborts_at.expect("X's first move has its window") - waiting.at;
    assert!(
        window <= u64::from(ABORT) && window > 0,
        "{window} ms to move"
    );

    let first = timed_step(&mut table, 0, moves[0]).await;
    assert_eq!((first.x_ms, first.o_ms, first.running), (BANK, BANK, None));
    assert!(first.aborts_at.is_some(), "O's first move has its window");
    let second = timed_step(&mut table, 1, moves[1]).await;
    assert_eq!(
        (second.x_ms, second.o_ms),
        (BANK, BANK),
        "no increment on a first move"
    );
    assert_eq!((second.running, second.aborts_at), (Some(Player::X), None));

    sleep(ms(100)).await;
    let third = timed_step(&mut table, 2, moves[2]).await;
    let most = BANK - 100 + INCREMENT;
    assert!(
        third.x_ms <= most && third.x_ms >= most - SLACK,
        "X spent about 100 ms and earned {INCREMENT}: {} ms",
        third.x_ms
    );
    assert_eq!((third.o_ms, third.running), (BANK, Some(Player::O)));
    assert!(
        third.at.abs_diff(now_ms()) < 5_000,
        "`at` is the server's Unix ms"
    );
}

#[tokio::test]
async fn a_running_clock_loses_on_time() {
    let world = world().await;
    let mut table = timed(&world, TimeControl::ThreePlusTwo).await;
    let moves = cells(TOP_ROW);
    timed_step(&mut table, 0, moves[0]).await;
    timed_step(&mut table, 1, moves[1]).await;
    let started = Instant::now();
    both_hear(&mut table, &ended(GameResult::OWins, Termination::Timeout)).await;
    let waited = started.elapsed();
    assert!(
        waited >= ms(BANK - 50),
        "X's whole bank ran out first: {waited:?}"
    );

    step(&mut table.x, 2, moves[2]).await;
    assert_eq!(
        hear(&mut table.x).await,
        ended(GameResult::OWins, Termination::Timeout),
        "a move after the flag finds the game over"
    );
    let (result, termination): (Option<String>, Option<String>) =
        sqlx::query_as("SELECT result, termination FROM games WHERE room = $1")
            .bind(&table.room)
            .fetch_one(&world.pool)
            .await
            .unwrap();
    assert_eq!(
        (result.as_deref(), termination.as_deref()),
        (Some("0-1"), Some("timeout"))
    );
    let (_, _, clocks) = rejoined(&world, &table.joiner, &table.room).await;
    assert_eq!((clocks.x_ms, clocks.o_ms, clocks.running), (0, BANK, None));
}

#[tokio::test]
async fn either_player_can_lose_on_time() {
    let world = world().await;
    let mut table = timed(&world, TimeControl::ThreePlusTwo).await;
    let moves = cells(TOP_ROW);
    timed_step(&mut table, 0, moves[0]).await;
    timed_step(&mut table, 1, moves[1]).await;
    let x = timed_step(&mut table, 2, moves[2]).await.x_ms;
    both_hear(&mut table, &ended(GameResult::XWins, Termination::Timeout)).await;
    let (_, _, clocks) = rejoined(&world, &table.host, &table.room).await;
    assert_eq!((clocks.x_ms, clocks.o_ms, clocks.running), (x, 0, None));
}

#[tokio::test]
async fn a_first_move_not_made_aborts_the_game() {
    let world = world().await;
    let mut table = timed(&world, TimeControl::ThreePlusTwo).await;
    both_hear(&mut table, &ABORTED).await;
    for socket in [&mut table.x, &mut table.o] {
        assert_eq!(closed(socket).await, 1011, "the Room is gone");
    }
    assert_eq!(games(&world.pool, &table.room).await, 0, "nothing is kept");
    let mut again = world.enter(&table.host, &table.room).await;
    assert_eq!(closed(&mut again).await, CloseCode::NoRoom.code());

    // O's first move has its own window, from X's.
    let mut table = timed(&world, TimeControl::ThreePlusTwo).await;
    let moves = cells(TOP_ROW);
    timed_step(&mut table, 0, moves[0]).await;
    let started = Instant::now();
    both_hear(&mut table, &ABORTED).await;
    assert!(started.elapsed() >= ms(ABORT - 50));
    assert_eq!(games(&world.pool, &table.room).await, 0);
}

#[tokio::test]
async fn a_timed_room_waits_for_its_second_player() {
    let world = world().await;
    let (host, joiner) = (
        common::guest(&world.app).await,
        common::guest(&world.app).await,
    );
    let body = r#"{"rules":"std","seat":"X","clock":"3+2"}"#;
    let made = create(&world.app, Some(&host), body).await;
    let room = made.json["room"].as_str().unwrap().to_owned();
    let mut x = world.enter(&host, &room).await;
    let (_, waiting) = welcome(&mut x).await;
    assert_eq!(waiting.clocks.unwrap().aborts_at, None, "no turn yet");
    sleep(ms(ABORT * 2)).await;

    let mut o = world.enter(&joiner, &room).await;
    for socket in [&mut x, &mut o] {
        let (_, snapshot) = welcome(socket).await;
        let clocks = snapshot.clocks.unwrap();
        let window = clocks.aborts_at.expect("X's first move begins its window") - clocks.at;
        assert!(
            window > u64::from(ABORT - SLACK),
            "a full window: {window} ms"
        );
    }
    let mut table = Table {
        room,
        host,
        joiner,
        x,
        o,
        players: waiting.players,
    };
    timed_step(&mut table, 0, cells(TOP_ROW)[0]).await;
}

#[tokio::test]
async fn an_untimed_room_never_aborts() {
    let world = world().await;
    let mut table = world.table("std").await;
    sleep(ms(ABORT * 2)).await;
    play(&mut table, 0, &cells(TOP_ROW)[..2]).await;
    let (_, snapshot) = welcome(&mut world.enter(&table.host, &table.room).await).await;
    assert_eq!((snapshot.clock, snapshot.clocks), (None, None));
}

/// `by` asks to take back their last move and the other player accepts: the clocks after it.
async fn taken_back(table: &mut Table, by: Player, to_ply: u16) -> Clocks {
    say(table.socket(by), &ClientMsg::AskTakeback).await;
    for socket in [&mut table.x, &mut table.o] {
        assert!(matches!(hear(socket).await, ServerMsg::Offer(_)));
    }
    let answer = ClientMsg::AnswerTakeback { accept: true };
    say(table.socket(by.opponent()), &answer).await;
    let mut heard = Vec::new();
    for socket in [&mut table.x, &mut table.o] {
        let accepted = ServerMsg::OfferAnswered {
            kind: OfferKind::Takeback,
            accepted: true,
        };
        assert_eq!(hear(socket).await, accepted);
        match hear(socket).await {
            ServerMsg::TookBack {
                to_ply: at,
                clocks: Some(clocks),
            } if at == to_ply => heard.push(clocks),
            other => panic!("not the takeback to ply {to_ply}: {other:?}"),
        }
    }
    heard[0]
}

#[tokio::test]
async fn a_takeback_restores_both_clocks_and_a_restart_keeps_them() {
    let mut world = world().await;
    let mut table = timed(&world, TimeControl::FifteenPlusFive).await;
    let moves = cells(TOP_ROW);
    timed_step(&mut table, 0, moves[0]).await;
    timed_step(&mut table, 1, moves[1]).await;
    sleep(ms(100)).await;
    timed_step(&mut table, 2, moves[2]).await;
    sleep(ms(100)).await;
    let spent = timed_step(&mut table, 3, moves[3]).await;
    assert!(spent.x_ms < LONG_BANK + LONG_INCREMENT && spent.o_ms < LONG_BANK + LONG_INCREMENT);

    let clocks = taken_back(&mut table, Player::X, 2).await;
    assert_eq!(
        (clocks.x_ms, clocks.o_ms, clocks.running),
        (LONG_BANK, LONG_BANK, Some(Player::X)),
        "both clocks as they were after the move kept"
    );

    restart(&mut world, Duration::ZERO).await;
    let (_, _, clocks) = rejoined(&world, &table.host, &table.room).await;
    assert_eq!((clocks.o_ms, clocks.running), (LONG_BANK, Some(Player::X)));
    assert!(
        clocks.x_ms <= LONG_BANK && clocks.x_ms > LONG_BANK / 2,
        "{} ms",
        clocks.x_ms
    );
}

#[tokio::test]
async fn a_first_move_taken_back_reopens_its_window() {
    let world = world().await;
    let mut table = timed(&world, TimeControl::ThreePlusTwo).await;
    timed_step(&mut table, 0, cells(TOP_ROW)[0]).await;
    let clocks = taken_back(&mut table, Player::X, 0).await;
    assert_eq!(
        (clocks.x_ms, clocks.o_ms, clocks.running),
        (BANK, BANK, None)
    );
    let window = clocks.aborts_at.expect("X's first move again") - clocks.at;
    assert!(
        window > u64::from(ABORT - SLACK),
        "a whole window: {window} ms"
    );
}

#[tokio::test]
async fn a_clock_runs_on_through_a_reconnect_and_a_restart() {
    let mut world = world().await;
    let mut table = timed(&world, TimeControl::FifteenPlusFive).await;
    let moves = cells(TOP_ROW);
    timed_step(&mut table, 0, moves[0]).await;
    timed_step(&mut table, 1, moves[1]).await;
    sleep(ms(100)).await;
    timed_step(&mut table, 2, moves[2]).await;
    sleep(ms(100)).await;
    let played = timed_step(&mut table, 3, moves[3]).await;
    let most = LONG_BANK - 100 + LONG_INCREMENT;
    assert!(played.o_ms <= most && played.o_ms >= most - SLACK);

    sleep(ms(150)).await;
    let (_, _, clocks) = rejoined(&world, &table.host, &table.room).await;
    assert_eq!(
        (clocks.o_ms, clocks.running),
        (played.o_ms, Some(Player::X))
    );
    assert!(
        clocks.x_ms <= played.x_ms - 150 && clocks.x_ms >= played.x_ms - 150 - SLACK,
        "a reconnect sees X's clock counted down: {} ms of {}",
        clocks.x_ms,
        played.x_ms
    );

    // Nothing runs the Room while the server's down, and its clock still runs.
    restart(&mut world, ms(200)).await;
    let (_, _, after) = rejoined(&world, &table.host, &table.room).await;
    assert_eq!(after.o_ms, played.o_ms, "O's time left was kept");
    assert!(
        after.x_ms <= clocks.x_ms - 200 && after.x_ms > 0,
        "the time the server was down counts: {} ms",
        after.x_ms
    );
}

#[tokio::test]
async fn a_room_loaded_past_its_flag_ends_on_time() {
    let mut world = world().await;
    let mut table = timed(&world, TimeControl::ThreePlusTwo).await;
    let moves = cells(TOP_ROW);
    timed_step(&mut table, 0, moves[0]).await;
    timed_step(&mut table, 1, moves[1]).await;
    restart(&mut world, ms(BANK + 100)).await;
    let (_, snapshot, clocks) = rejoined(&world, &table.joiner, &table.room).await;
    assert_eq!(
        snapshot.end.map(|end| (end.result, end.termination)),
        Some((GameResult::OWins, Termination::Timeout))
    );
    assert_eq!((clocks.x_ms, clocks.running), (0, None));
}

#[tokio::test]
async fn an_aborted_rematch_lets_go_of_its_game() {
    let world = world().await;
    let mut table = timed(&world, TimeControl::ThreePlusTwo).await;
    for (ply, &cell) in (0..).zip(&cells(TOP_ROW)) {
        timed_step(&mut table, ply, cell).await;
    }
    both_hear(&mut table, &ended(GameResult::XWins, Termination::Line)).await;
    say(&mut table.x, &ClientMsg::OfferRematch).await;
    for socket in [&mut table.x, &mut table.o] {
        assert!(matches!(hear(socket).await, ServerMsg::Offer(_)));
    }
    say(&mut table.o, &ClientMsg::AnswerRematch { accept: true }).await;
    let mut next = None;
    for socket in [&mut table.x, &mut table.o] {
        hear(socket).await;
        let ServerMsg::RematchRoom { room } = hear(socket).await else {
            panic!("no rematch");
        };
        next = Some(room);
    }
    let next = next.unwrap();
    let card = card(&world.app, None, &next).await;
    assert_eq!(card.json["clock"], "3+2", "the rematch keeps the clock");

    // Both seats are filled as it's made, so its first move's window runs from then.
    sleep(ms(ABORT + 200)).await;
    let mut late = world.enter(&table.joiner, &next).await;
    assert_eq!(closed(&mut late).await, CloseCode::NoRoom.code());
    assert_eq!(games(&world.pool, &next).await, 0);
    let named: Option<String> = sqlx::query_scalar("SELECT rematch FROM games WHERE room = $1")
        .bind(&table.room)
        .fetch_one(&world.pool)
        .await
        .unwrap();
    assert_eq!(named, None, "the finished game lets go of it");
}

#[tokio::test]
async fn a_ping_gets_the_servers_time() {
    let world = world().await;
    let mut table = world.table("std").await;
    say(&mut table.x, &ClientMsg::Ping { t: 42 }).await;
    let ServerMsg::Pong { t, server_ms } = hear(&mut table.x).await else {
        panic!("no Pong");
    };
    assert_eq!(t, 42);
    assert!(server_ms.abs_diff(now_ms()) < 5_000);
    play(&mut table, 0, &cells(TOP_ROW)[..1]).await;
}

#[tokio::test]
async fn a_room_is_made_with_a_time_control() {
    let world = world().await;
    let host = common::guest(&world.app).await;
    for control in TimeControl::ALL {
        let body = format!(r#"{{"rules":"std","seat":"X","clock":"{control}"}}"#);
        let made = create(&world.app, Some(&host), &body).await;
        assert_eq!(made.status, StatusCode::CREATED);
        let room = made.json["room"].as_str().unwrap();
        let card = card(&world.app, None, room).await;
        assert_eq!(card.json["clock"], control.as_str());
    }
    let untimed = create(&world.app, Some(&host), r#"{"rules":"std","seat":"X"}"#).await;
    let room = untimed.json["room"].as_str().unwrap();
    assert_eq!(
        card(&world.app, None, room).await.json["clock"],
        serde_json::Value::Null
    );
    let refused = create(
        &world.app,
        Some(&host),
        r#"{"rules":"std","seat":"X","clock":"4+4"}"#,
    )
    .await;
    assert!(refused.status.is_client_error(), "{}", refused.status);
}
