//! Spectators (spec §10.2), on `common::TIMING`'s short time: a Spectator of a Room its host let
//! them watch hears what the players hear, live in a Casual Room, while nothing it asks is done,
//! and everyone hears how many watch, at most every 100 ms. A world whose Casual Rooms are watched
//! late stands in for a Rated game's 10 s.

mod common;

use std::time::Duration;

use common::table::*;
use common::{Socket, TIMING, closed, guest, hear, next, say};
use kokonoe_engine::Player;
use kokonoe_protocol::{
    Abort, Away, ClaimKind, ClientMsg, CloseCode, GameEnd, GameResult, OfferKind, Presence,
    Rejection, Role, ServerMsg, Termination,
};
use kokonoe_server::Timing;
use sqlx::PgPool;
use tokio::time::Instant;

/// A Room its host let Spectators watch, the host as X.
const WATCHED: &str = r#"{"rules":"std","seat":"X","spectators":true}"#;

fn watching(count: u32) -> ServerMsg {
    ServerMsg::Watching { count }
}

/// Closes `socket`, as a page does when it's closed.
async fn leave(socket: &mut Socket) {
    socket.close(None).await.unwrap();
}

/// `player` is away, the next thing `socket` hears.
async fn away(socket: &mut Socket, player: Player) -> Away {
    match hear(socket).await {
        ServerMsg::Presence {
            player: who,
            away: Some(away),
        } if who == player => away,
        other => panic!("not {player:?} away: {other:?}"),
    }
}

/// `socket` says `message`, and is told it isn't allowed.
async fn not_allowed(socket: &mut Socket, message: ClientMsg) {
    say(socket, &message).await;
    assert_eq!(
        hear(socket).await,
        rejected(Rejection::NotAllowed),
        "{message:?}"
    );
}

/// The Result and Termination the table holds for `room`.
async fn ending(pool: &PgPool, room: &str) -> (Option<String>, Option<String>) {
    sqlx::query_as("SELECT result, termination FROM games WHERE room = $1")
        .bind(room)
        .fetch_one(pool)
        .await
        .unwrap()
}

#[tokio::test]
async fn a_spectator_hears_every_move_live_and_may_make_none() {
    let world = world().await;
    let mut table = world.table_with(WATCHED).await;
    let moves = cells(TOP_ROW);
    play(&mut table, 0, &moves[..2]).await;

    let stranger = guest(&world.app).await;
    let (mut watcher, snapshot) = world.watch(&stranger, &table.room).await;
    assert_eq!(indices(&snapshot.record), moves[..2]);
    assert_eq!(snapshot.players, table.players);
    assert_eq!((snapshot.watching, snapshot.delay_ms), (1, 0), "live");
    for socket in [&mut table.x, &mut table.o] {
        assert_eq!(hear(socket).await, watching(1), "a count, not a name");
    }
    play(&mut table, 2, &moves[2..3]).await;
    assert_eq!(hear(&mut watcher).await, moved(2, moves[2]));

    step(&mut watcher, 3, moves[3]).await;
    assert_eq!(hear(&mut watcher).await, rejected(Rejection::NotAllowed));
    for message in [
        ClientMsg::Resign,
        ClientMsg::OfferDraw,
        ClientMsg::AnswerDraw { accept: true },
        ClientMsg::AskTakeback,
        ClientMsg::AnswerTakeback { accept: true },
        ClientMsg::OfferRematch,
        ClientMsg::AnswerRematch { accept: true },
        ClientMsg::Claim {
            kind: ClaimKind::Win,
        },
        ClientMsg::Claim {
            kind: ClaimKind::Draw,
        },
    ] {
        not_allowed(&mut watcher, message).await;
    }
    assert_eq!(
        stored(&world.pool, &table.room).await.len(),
        3,
        "nothing the Spectator sent was stored"
    );
    // The players heard none of it: O's move is what comes next, to everyone.
    play(&mut table, 3, &moves[3..4]).await;
    assert_eq!(hear(&mut watcher).await, moved(3, moves[3]));

    let asked = card(&world.app, Some(&stranger), &table.room).await;
    assert_eq!(asked.json["you"], serde_json::Value::Null, "no seat");
    assert_eq!(asked.json["spectators"], true);
    // A second Hello closes a Spectator's socket, as it does a player's.
    say(&mut watcher, &hello_as(&table.room, Role::Spectator)).await;
    assert_eq!(closed(&mut watcher).await, CloseCode::Forbidden.code());
}

#[tokio::test]
async fn everyone_hears_how_many_watch_as_they_come_and_go() {
    let world = world().await;
    let mut table = world.table_with(WATCHED).await;
    let (one, two) = (guest(&world.app).await, guest(&world.app).await);

    let (mut first, snapshot) = world.watch(&one, &table.room).await;
    assert_eq!(snapshot.watching, 1);
    for socket in [&mut table.x, &mut table.o] {
        assert_eq!(hear(socket).await, watching(1));
    }
    // A second socket of the same Account's is still one Spectator, and changes nothing.
    let (mut again, snapshot) = world.watch(&one, &table.room).await;
    assert_eq!(snapshot.watching, 1);
    let (mut second, snapshot) = world.watch(&two, &table.room).await;
    assert_eq!(snapshot.watching, 2);
    for socket in [&mut table.x, &mut table.o, &mut first, &mut again] {
        assert_eq!(hear(socket).await, watching(2));
    }

    // One Account's first socket going leaves it watching from the other.
    leave(&mut first).await;
    leave(&mut second).await;
    for socket in [&mut table.x, &mut table.o, &mut again] {
        assert_eq!(hear(socket).await, watching(1));
    }
    leave(&mut again).await;
    for socket in [&mut table.x, &mut table.o] {
        assert_eq!(hear(socket).await, watching(0));
    }
    // Spectators going is no one away: the next thing the players hear is a move.
    play(&mut table, 0, &cells(TOP_ROW)[..1]).await;
}

#[tokio::test]
async fn a_room_its_host_kept_closed_turns_spectators_away() {
    let world = world().await;
    // Spectators are off unless the host asks for them.
    let mut table = world.table("std").await;
    let stranger = guest(&world.app).await;
    let asked = card(&world.app, Some(&stranger), &table.room).await;
    assert_eq!(asked.json["spectators"], false);

    let mut watcher = world.spectate(&stranger, &table.room).await;
    assert_eq!(closed(&mut watcher).await, CloseCode::Forbidden.code());
    // To a player's Hello it's as full as ever.
    let mut player = world.enter(&stranger, &table.room).await;
    assert_eq!(closed(&mut player).await, CloseCode::Forbidden.code());
    // The players heard nothing of either: the next thing is a move.
    play(&mut table, 0, &cells(TOP_ROW)[..1]).await;
}

#[tokio::test]
async fn a_spectator_takes_no_seat_and_hears_who_does() {
    let world = world().await;
    let (host, joiner, stranger) = (
        guest(&world.app).await,
        guest(&world.app).await,
        guest(&world.app).await,
    );
    let made = create(&world.app, Some(&host), WATCHED).await;
    let room = made.json["room"].as_str().unwrap().to_owned();
    let mut x = world.enter(&host, &room).await;
    welcomed(&mut x).await;

    let (mut watcher, snapshot) = world.watch(&stranger, &room).await;
    assert_eq!(snapshot.players.o, None, "the seat stays empty");
    assert_eq!(hear(&mut x).await, watching(1));
    let asked = card(&world.app, Some(&joiner), &room).await;
    assert_eq!(asked.json["players"]["O"], serde_json::Value::Null);

    let mut o = world.enter(&joiner, &room).await;
    let (you, _, players) = welcomed(&mut o).await;
    assert_eq!(
        you,
        Player::O,
        "the joiner takes the seat the Spectator didn't"
    );
    assert_eq!(welcomed(&mut x).await.2, players);
    let snapshot = watched(&mut watcher).await;
    assert_eq!(snapshot.players, players, "the Spectator hears who joined");
    assert_eq!(snapshot.watching, 1);
}

#[tokio::test]
async fn an_account_with_a_seat_comes_as_its_player() {
    let world = world().await;
    let (host, friend) = (guest(&world.app).await, guest(&world.app).await);
    let made = create(&world.app, Some(&host), WATCHED).await;
    let room = made.json["room"].as_str().unwrap().to_owned();
    let mut x = world.enter(&host, &room).await;
    welcomed(&mut x).await;

    // The host's Spectator Hello takes their own seat.
    let mut second = world.spectate(&host, &room).await;
    assert_eq!(welcome(&mut second).await.0, Player::X);

    // A friend who watched, then took the free seat, plays from every page.
    let (mut watching_page, _) = world.watch(&friend, &room).await;
    assert_eq!(hear(&mut x).await, watching(1));
    assert_eq!(hear(&mut second).await, watching(1));
    let mut o = world.enter(&friend, &room).await;
    assert_eq!(welcome(&mut o).await.0, Player::O);
    assert_eq!(
        closed(&mut watching_page).await,
        1011,
        "its watching page reconnects"
    );
    let mut back = world.spectate(&friend, &room).await;
    let (you, snapshot) = welcome(&mut back).await;
    assert_eq!(you, Player::O);
    assert_eq!(snapshot.watching, 0, "no one watches now");
}

#[tokio::test]
async fn spectators_hear_both_players_go_and_keep_neither_here() {
    let world = world().await;
    let mut table = world.table_with(WATCHED).await;
    let moves = cells(TOP_ROW);
    play(&mut table, 0, &moves[..2]).await;
    let stranger = guest(&world.app).await;
    let (mut watcher, snapshot) = world.watch(&stranger, &table.room).await;
    assert_eq!(snapshot.presence, Presence::default(), "both players here");
    for socket in [&mut table.x, &mut table.o] {
        assert_eq!(hear(socket).await, watching(1));
    }

    leave(&mut table.x).await;
    let gone = away(&mut table.o, Player::X).await;
    assert_eq!(away(&mut watcher, Player::X).await.since, gone.since);
    leave(&mut table.o).await;
    away(&mut watcher, Player::O).await;

    // A game both players left ends Abandoned, however many watch it, and they hear so.
    let left = Instant::now();
    assert_eq!(
        hear(&mut watcher).await,
        ServerMsg::Ended(GameEnd::ABANDONED)
    );
    assert!(left.elapsed() >= TIMING.abandon - Duration::from_millis(150));
    assert_eq!(
        ending(&world.pool, &table.room).await,
        (None, Some("abandoned".into()))
    );

    // Nor does a Spectator keep a player here for their opponent's Claim.
    let mut table = world.table_with(WATCHED).await;
    let (mut watcher, _) = world.watch(&stranger, &table.room).await;
    for socket in [&mut table.x, &mut table.o] {
        assert_eq!(hear(socket).await, watching(1));
    }
    play(&mut table, 0, &moves[..2]).await;
    for ply in 0..2 {
        assert_eq!(
            hear(&mut watcher).await,
            moved(ply, moves[usize::from(ply)])
        );
    }
    leave(&mut table.x).await;
    let gone = away(&mut table.o, Player::X).await;
    away(&mut watcher, Player::X).await;
    tokio::time::sleep(Duration::from_millis(gone.claims_in.into())).await;
    say(
        &mut table.o,
        &ClientMsg::Claim {
            kind: ClaimKind::Win,
        },
    )
    .await;
    let won = ended(GameResult::OWins, Termination::Claim);
    assert_eq!(hear(&mut table.o).await, won);
    assert_eq!(hear(&mut watcher).await, won);
}

#[tokio::test]
async fn a_spectator_follows_the_rematch_to_a_room_it_can_watch() {
    let world = world().await;
    let mut table = world.table_with(WATCHED).await;
    let stranger = guest(&world.app).await;
    let (mut watcher, _) = world.watch(&stranger, &table.room).await;
    for socket in [&mut table.x, &mut table.o] {
        assert_eq!(hear(socket).await, watching(1));
    }

    say(&mut table.x, &ClientMsg::Resign).await;
    let resigned = ended(GameResult::OWins, Termination::Resign);
    for socket in [&mut table.x, &mut table.o, &mut watcher] {
        assert_eq!(hear(socket).await, resigned);
    }
    say(&mut table.o, &ClientMsg::OfferRematch).await;
    for socket in [&mut table.x, &mut table.o, &mut watcher] {
        assert!(matches!(
            hear(socket).await,
            ServerMsg::Offer(offer) if offer.kind == OfferKind::Rematch && offer.by == Player::O
        ));
    }
    say(&mut table.x, &ClientMsg::AnswerRematch { accept: true }).await;
    let mut rooms = Vec::new();
    for socket in [&mut table.x, &mut table.o, &mut watcher] {
        let accepted = ServerMsg::OfferAnswered {
            kind: OfferKind::Rematch,
            accepted: true,
        };
        assert_eq!(hear(socket).await, accepted);
        match hear(socket).await {
            ServerMsg::RematchRoom { room } => rooms.push(room),
            other => panic!("not the rematch: {other:?}"),
        }
    }
    assert!(rooms.iter().all(|room| room == &rooms[0]));

    let next = &rooms[0];
    let asked = card(&world.app, Some(&stranger), next).await;
    assert_eq!(
        asked.json["spectators"], true,
        "the rematch can be watched too"
    );
    let (_, snapshot) = world.watch(&stranger, next).await;
    assert_eq!(
        (snapshot.players.x, snapshot.players.o),
        (table.players.o, table.players.x),
        "X and O swapped"
    );
}

/// How late a world whose Casual Rooms are watched late has its Spectators hear them.
const LATE: Duration = Duration::from_millis(300);

#[tokio::test]
async fn a_late_spectator_hears_everything_its_delay_after_the_players() {
    let world = world_with(Timing {
        casual_delay: LATE,
        ..TIMING
    })
    .await;
    let mut table = world.table_with(WATCHED).await;
    let stranger = guest(&world.app).await;

    let asked = Instant::now();
    let mut watcher = world.spectate(&stranger, &table.room).await;
    let snapshot = watched(&mut watcher).await;
    assert!(asked.elapsed() >= LATE, "its Welcome comes late too");
    assert_eq!(snapshot.delay_ms, 300);
    for socket in [&mut table.x, &mut table.o] {
        assert_eq!(hear(socket).await, watching(1), "the players hear it live");
    }

    // Timers never fire early, so nothing can reach the Spectator sooner than its delay after
    // the move was sent.
    let moves = cells(TOP_ROW);
    let sent = Instant::now();
    play(&mut table, 0, &moves[..2]).await;
    let early =
        tokio::time::timeout_at(sent + LATE - Duration::from_millis(50), next(&mut watcher));
    assert!(early.await.is_err(), "nothing yet");
    assert_eq!(hear(&mut watcher).await, moved(0, moves[0]));
    assert!(sent.elapsed() >= LATE);
    assert_eq!(hear(&mut watcher).await, moved(1, moves[1]), "in order");
    let (_, snapshot) = world.rejoin(&table.host, &table.room).await;
    assert_eq!(snapshot.delay_ms, 0, "a player hears the Room live");

    // What the Room says before it closes a late Spectator's socket still reaches it, late.
    let made = Instant::now();
    let mut timed = world
        .table_with(r#"{"rules":"std","seat":"X","clock":"3+2","spectators":true}"#)
        .await;
    let mut watcher = world.spectate(&stranger, &timed.room).await;
    watched(&mut watcher).await;
    for socket in [&mut timed.x, &mut timed.o] {
        assert_eq!(hear(socket).await, watching(1));
    }
    let aborted = ServerMsg::Aborted {
        reason: Abort::FirstMoveTimeout,
    };
    for socket in [&mut timed.x, &mut timed.o] {
        assert_eq!(hear(socket).await, aborted);
    }
    assert_eq!(hear(&mut watcher).await, aborted);
    // The first move's window opened after `made`, and the abort came its delay after it closed.
    assert!(made.elapsed() >= TIMING.abort + LATE);
    assert_eq!(closed(&mut watcher).await, 1011, "the Room is gone");
}
