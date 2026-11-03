//! Who is in a Room, and what comes of their leaving (spec §10.3 and §11), on `common::TIMING`'s
//! short time: a Claim comes due 700 ms after the other player's last socket went, a game both
//! players left ends Abandoned 1.5 s on, and the sweep looks for one every 100 ms.

mod common;

use std::time::Duration;

use common::table::*;
use common::{Server, Socket, TIMING, eventually, guest, hear, say};
use kokonoe_engine::Player;
use kokonoe_protocol::{
    Away, ClaimKind, ClientMsg, GameEnd, GameResult, OfferKind, Rejection, ServerMsg, Termination,
};
use kokonoe_server::Timing;
use sqlx::PgPool;
use tokio::time::{Instant, sleep};

/// How long the sweep may take to find and settle a Room past its wait.
const SWEPT: Duration = Duration::from_millis(1_000);

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

fn back(player: Player) -> ServerMsg {
    ServerMsg::Presence { player, away: None }
}

fn claim(kind: ClaimKind) -> ClientMsg {
    ClientMsg::Claim { kind }
}

fn ms(duration: Duration) -> u32 {
    u32::try_from(duration.as_millis()).unwrap()
}

/// The Result and Termination the table holds for `room`.
async fn ending(pool: &PgPool, room: &str) -> (Option<String>, Option<String>) {
    sqlx::query_as("SELECT result, termination FROM games WHERE room = $1")
        .bind(room)
        .fetch_one(pool)
        .await
        .unwrap()
}

/// Whether the table holds each player away.
async fn aways(pool: &PgPool, room: &str) -> (bool, bool) {
    sqlx::query_as("SELECT x_away IS NOT NULL, o_away IS NOT NULL FROM games WHERE room = $1")
        .bind(room)
        .fetch_one(pool)
        .await
        .unwrap()
}

/// How the game in `room` ended, once the sweep has ended it.
async fn swept(pool: &PgPool, room: &str) -> (Option<String>, Option<String>) {
    eventually(
        TIMING.abandon + SWEPT,
        "the sweep ended the game",
        async || {
            let found = ending(pool, room).await;
            found.1.is_some().then_some(found)
        },
    )
    .await
}

#[tokio::test]
async fn a_player_is_away_once_their_last_socket_leaves_and_back_with_their_first() {
    let world = world().await;
    let mut table = world.table("std").await;
    let moves = cells(TOP_ROW);
    assert_eq!(aways(&world.pool, &table.room).await, (false, false));

    let mut second = world.enter(&table.host, &table.room).await;
    welcome(&mut second).await;
    leave(&mut table.x).await;
    step(&mut second, 0, moves[0]).await;
    assert_eq!(
        hear(&mut table.o).await,
        moved(0, moves[0]),
        "X is still here on another socket"
    );
    assert_eq!(hear(&mut second).await, moved(0, moves[0]));

    leave(&mut second).await;
    let gone = away(&mut table.o, Player::X).await;
    assert_eq!(
        gone.claims_in,
        ms(TIMING.claim),
        "a Claim waits its whole wait"
    );
    assert_eq!(aways(&world.pool, &table.room).await, (true, false));

    let (mut watching, snapshot) = world.rejoin(&table.joiner, &table.room).await;
    assert!(snapshot.presence.x.is_some(), "a Welcome says X is away");
    assert_eq!(snapshot.presence.o, None);

    table.x = world.enter(&table.host, &table.room).await;
    let (_, snapshot) = welcome(&mut table.x).await;
    assert_eq!(snapshot.presence.x, None, "X's own Welcome has X here");
    assert_eq!(hear(&mut table.o).await, back(Player::X));
    assert_eq!(hear(&mut watching).await, back(Player::X));
    assert_eq!(aways(&world.pool, &table.room).await, (false, false));
}

#[tokio::test]
async fn a_claim_is_refused_until_the_other_player_has_been_away_long_enough() {
    let world = world().await;
    let mut table = world.table("std").await;
    let refused = rejected(Rejection::NotAllowed);
    say(&mut table.x, &claim(ClaimKind::Win)).await;
    assert_eq!(hear(&mut table.x).await, refused, "while O is here");

    leave(&mut table.o).await;
    away(&mut table.x, Player::O).await;
    say(&mut table.x, &claim(ClaimKind::Win)).await;
    assert_eq!(hear(&mut table.x).await, refused, "before the wait");

    sleep(TIMING.claim).await;
    say(&mut table.x, &claim(ClaimKind::Win)).await;
    let won = ended(GameResult::XWins, Termination::Claim);
    assert_eq!(hear(&mut table.x).await, won);
    assert_eq!(
        ending(&world.pool, &table.room).await,
        (Some("1-0".into()), Some("claim".into()))
    );
    say(&mut table.x, &claim(ClaimKind::Win)).await;
    assert_eq!(hear(&mut table.x).await, refused, "once the game is over");

    let (_, snapshot) = world.rejoin(&table.joiner, &table.room).await;
    assert_eq!(
        snapshot.end,
        Some(GameEnd {
            result: Some(GameResult::XWins),
            termination: Termination::Claim
        }),
        "a claimed win is an ordinary loss, kept"
    );
}

#[tokio::test]
async fn a_claim_can_end_the_game_drawn() {
    let world = world().await;
    let mut table = world.table("std").await;
    leave(&mut table.x).await;
    away(&mut table.o, Player::X).await;
    sleep(TIMING.claim).await;
    say(&mut table.o, &claim(ClaimKind::Draw)).await;
    assert_eq!(
        hear(&mut table.o).await,
        ended(GameResult::Draw, Termination::Claim)
    );
    assert_eq!(
        ending(&world.pool, &table.room).await,
        (Some("1/2-1/2".into()), Some("claim".into()))
    );
}

#[tokio::test]
async fn a_claim_after_a_restart_counts_the_time_already_away() {
    let mut world = world().await;
    let mut table = world.table("std").await;
    leave(&mut table.o).await;
    away(&mut table.x, Player::O).await;

    world.server.kill();
    sleep(TIMING.claim).await;
    world.server = Server::start_with(&world.url, world.timing).await;
    table.x = world.enter(&table.host, &table.room).await;
    let (_, snapshot) = welcome(&mut table.x).await;
    let o = snapshot.presence.o.expect("O is still away");
    assert_eq!(o.claims_in, 0, "O was away across the restart");
    say(&mut table.x, &claim(ClaimKind::Win)).await;
    assert_eq!(
        hear(&mut table.x).await,
        ended(GameResult::XWins, Termination::Claim)
    );
}

#[tokio::test]
async fn the_absent_players_clock_runs_on_until_a_claim_stops_it() {
    let world = world().await;
    let mut table = world
        .table_with(r#"{"rules":"std","seat":"X","clock":"15+5"}"#)
        .await;
    let moves = cells(TOP_ROW);
    timed_play(&mut table, 0, &moves[..4]).await;
    // X to move, with their clock running.
    leave(&mut table.x).await;
    away(&mut table.o, Player::X).await;
    sleep(TIMING.claim).await;

    let (mut watching, snapshot) = world.rejoin(&table.joiner, &table.room).await;
    let clocks = snapshot.clocks.unwrap();
    assert_eq!(
        clocks.running,
        Some(Player::X),
        "X's clock runs while X is away"
    );
    // 15+5 here is a 3 s bank and a 200 ms increment.
    assert!(clocks.x_ms <= 3_200 - ms(TIMING.claim), "{clocks:?}");

    say(&mut table.o, &claim(ClaimKind::Win)).await;
    let won = ended(GameResult::OWins, Termination::Claim);
    assert_eq!(hear(&mut table.o).await, won);
    assert_eq!(hear(&mut watching).await, won);
}

#[tokio::test]
async fn an_absent_player_whose_clock_runs_out_loses_on_time_first() {
    let world = world().await;
    let mut table = world
        .table_with(r#"{"rules":"std","seat":"X","clock":"3+2"}"#)
        .await;
    let moves = cells(TOP_ROW);
    timed_play(&mut table, 0, &moves[..2]).await;
    // X to move, with 3+2's 600 ms bank: less than a Claim's wait.
    leave(&mut table.x).await;
    away(&mut table.o, Player::X).await;
    assert_eq!(
        hear(&mut table.o).await,
        ended(GameResult::OWins, Termination::Timeout)
    );
    say(&mut table.o, &claim(ClaimKind::Win)).await;
    assert_eq!(hear(&mut table.o).await, rejected(Rejection::NotAllowed));
}

#[tokio::test]
async fn an_empty_room_ends_abandoned_after_its_wait() {
    let world = world().await;
    let mut table = world.table("std").await;
    let moves = cells(TOP_ROW);
    play(&mut table, 0, &moves[..2]).await;
    leave(&mut table.x).await;
    away(&mut table.o, Player::X).await;
    leave(&mut table.o).await;
    assert_eq!(ending(&world.pool, &table.room).await, (None, None));

    let started = Instant::now();
    let found = swept(&world.pool, &table.room).await;
    let took = started.elapsed();
    assert_eq!(found, (None, Some("abandoned".into())), "no result");
    assert!(
        took >= TIMING.abandon - Duration::from_millis(150),
        "{took:?}"
    );
    assert_eq!(
        stored(&world.pool, &table.room).await.len(),
        2,
        "its moves kept"
    );

    table.x = world.enter(&table.host, &table.room).await;
    let (_, snapshot) = welcome(&mut table.x).await;
    assert_eq!(snapshot.end, Some(GameEnd::ABANDONED));
    step(&mut table.x, 2, moves[2]).await;
    assert_eq!(
        hear(&mut table.x).await,
        ServerMsg::Ended(GameEnd::ABANDONED)
    );
    say(&mut table.x, &claim(ClaimKind::Win)).await;
    assert_eq!(hear(&mut table.x).await, rejected(Rejection::NotAllowed));

    // Both may still play another game.
    table.o = world.enter(&table.joiner, &table.room).await;
    welcome(&mut table.o).await;
    assert_eq!(hear(&mut table.x).await, back(Player::O));
    say(&mut table.x, &ClientMsg::OfferRematch).await;
    for socket in [&mut table.x, &mut table.o] {
        assert!(matches!(
            hear(socket).await,
            ServerMsg::Offer(offer) if offer.kind == OfferKind::Rematch
        ));
    }
    say(&mut table.o, &ClientMsg::AnswerRematch { accept: true }).await;
    for socket in [&mut table.x, &mut table.o] {
        hear(socket).await;
        assert!(matches!(hear(socket).await, ServerMsg::RematchRoom { .. }));
    }
}

#[tokio::test]
async fn an_empty_timed_room_ends_on_time_or_aborts_instead() {
    let world = world().await;
    let mut table = world
        .table_with(r#"{"rules":"std","seat":"X","clock":"3+2"}"#)
        .await;
    timed_play(&mut table, 0, &cells(TOP_ROW)[..2]).await;
    leave(&mut table.x).await;
    away(&mut table.o, Player::X).await;
    leave(&mut table.o).await;
    assert_eq!(
        swept(&world.pool, &table.room).await,
        (Some("0-1".into()), Some("timeout".into())),
        "X's clock ran on"
    );

    let mut table = world
        .table_with(r#"{"rules":"std","seat":"X","clock":"3+2"}"#)
        .await;
    leave(&mut table.x).await;
    away(&mut table.o, Player::X).await;
    leave(&mut table.o).await;
    let aborted = "no first move: aborted, so not kept";
    eventually(TIMING.abandon + SWEPT, aborted, async || {
        (games(&world.pool, &table.room).await == 0).then_some(())
    })
    .await;
}

#[tokio::test]
async fn a_room_one_player_stays_in_is_never_abandoned() {
    let world = world().await;
    let mut table = world.table("std").await;
    leave(&mut table.o).await;
    away(&mut table.x, Player::O).await;
    sleep(TIMING.abandon + 3 * TIMING.sweep).await;
    assert_eq!(ending(&world.pool, &table.room).await, (None, None));
    say(&mut table.x, &claim(ClaimKind::Win)).await;
    assert_eq!(
        hear(&mut table.x).await,
        ended(GameResult::XWins, Termination::Claim)
    );
}

#[tokio::test]
async fn an_invite_no_one_accepted_is_never_abandoned() {
    let world = world().await;
    let host = guest(&world.app).await;
    let id = room(&world.app, &host, "std", "X").await;
    let mut socket = world.enter(&host, &id).await;
    welcome(&mut socket).await;
    leave(&mut socket).await;
    sleep(TIMING.abandon + 3 * TIMING.sweep).await;
    assert_eq!(ending(&world.pool, &id).await, (None, None));

    let joiner = guest(&world.app).await;
    let mut o = world.enter(&joiner, &id).await;
    let (you, snapshot) = welcome(&mut o).await;
    assert_eq!(you, Player::O);
    assert!(snapshot.players.x.is_some() && snapshot.players.o.is_some());
}

#[tokio::test]
async fn the_sweep_leaves_a_room_someone_is_in() {
    let world = world().await;
    let mut table = world.table("std").await;
    sqlx::query(
        "UPDATE games SET x_away = now() - interval '1 hour', o_away = now() - interval '1 hour'
         WHERE room = $1",
    )
    .bind(&table.room)
    .execute(&world.pool)
    .await
    .unwrap();
    sleep(3 * TIMING.sweep).await;
    assert_eq!(ending(&world.pool, &table.room).await, (None, None));
    play(&mut table, 0, &cells(TOP_ROW)[..1]).await;
}

#[tokio::test]
async fn a_room_loaded_past_its_wait_is_abandoned_before_anyone_plays() {
    let world = world_with(Timing {
        sweep: Duration::from_secs(3600),
        ..TIMING
    })
    .await;
    let mut table = world.table("std").await;
    play(&mut table, 0, &cells(TOP_ROW)[..1]).await;
    leave(&mut table.x).await;
    away(&mut table.o, Player::X).await;
    leave(&mut table.o).await;
    sleep(TIMING.abandon + Duration::from_millis(200)).await;
    assert_eq!(
        ending(&world.pool, &table.room).await,
        (None, None),
        "no sweep has looked"
    );

    let (_, snapshot) = world.rejoin(&table.host, &table.room).await;
    assert_eq!(snapshot.end, Some(GameEnd::ABANDONED));
}

#[tokio::test]
async fn a_socket_that_stops_answering_pings_is_closed_and_its_player_away() {
    let world = world_with(Timing {
        keepalive: Duration::from_millis(200),
        ..TIMING
    })
    .await;
    let mut table = world.table("std").await;
    // X's socket is never read again, so it answers none of the server's pings.
    away(&mut table.o, Player::X).await;
    assert_eq!(aways(&world.pool, &table.room).await, (true, false));
}

#[tokio::test]
async fn the_table_keeps_no_result_only_for_an_abandoned_game() {
    let world = world().await;
    let table = world.table("std").await;
    let set = |query: &'static str| {
        let pool = world.pool.clone();
        let room = table.room.clone();
        async move { sqlx::query(query).bind(room).execute(&pool).await }
    };
    let resigned = set("UPDATE games SET termination = 'resign' WHERE room = $1");
    assert!(resigned.await.is_err(), "a resignation needs a result");
    let abandoned_won =
        set("UPDATE games SET result = '1-0', termination = 'abandoned' WHERE room = $1");
    assert!(abandoned_won.await.is_err(), "an abandonment has none");
    let abandoned = set("UPDATE games SET termination = 'abandoned' WHERE room = $1");
    assert!(abandoned.await.is_ok());

    let host = guest(&world.app).await;
    let invite = room(&world.app, &host, "std", "X").await;
    let away = sqlx::query("UPDATE games SET o_away = now() WHERE room = $1")
        .bind(invite)
        .execute(&world.pool)
        .await;
    assert!(away.is_err(), "an empty seat's player can't be away");
}
