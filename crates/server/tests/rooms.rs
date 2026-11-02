//! Invite Rooms (spec §10) against a real Postgres in Docker: making one, describing it to a
//! joiner, and playing it over two sockets on a server that can be killed and started again.

mod common;

use std::time::Duration;

use axum::http::StatusCode;
use common::table::*;
use common::{Server, closed, guest, hear, open, say, session_id};
use futures_util::{SinkExt, StreamExt};
use kokonoe_engine::{Depth, Player, Preset, RULES_VERSION};
use kokonoe_protocol::{
    ClientMsg, GameEnd, GameResult, PROTOCOL, Players, Rejection, Role, ServerMsg, Termination,
};
use tokio_tungstenite::tungstenite::Message;

#[tokio::test]
async fn creating_a_room_needs_a_session_and_a_preset() {
    let world = world().await;
    let standard = r#"{"rules":"std","seat":"X"}"#;
    assert_eq!(
        create(&world.app, None, standard).await.status,
        StatusCode::FORBIDDEN
    );
    let host = guest(&world.app).await;
    let refused = [
        r#"{"rules":"standard","seat":"X"}"#,
        r#"{"rules":"cont+both","seat":"X"}"#,
        r#"{"rules":"std","seat":"Either"}"#,
        r#"{"rules":"std"}"#,
    ];
    for body in refused {
        let answer = create(&world.app, Some(&host), body).await;
        assert_eq!(answer.status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");
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
        room(&world.app, &host, "std", "Random").await;
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
    let (host, joiner) = (guest(&world.app).await, guest(&world.app).await);
    let room = room(&world.app, &host, "std", "O").await;
    let name = handle(&world.pool, &host).await;

    let asked = card(&world.app, Some(&joiner), &room).await;
    assert_eq!(asked.status, StatusCode::OK);
    assert_eq!(asked.cache.as_deref(), Some("no-store"));
    assert_eq!(
        asked.json,
        serde_json::json!({
            "host": name,
            "variant": "Standard",
            "rules": "std",
            "players": { "X": null, "O": name },
            "you": null,
            "takebacks": false,
            "clock": null,
        })
    );
    assert_eq!(card(&world.app, Some(&host), &room).await.json["you"], "O");

    for missing in ["00000000000000000000", "abc", "6F1C0A9B3E2D4C5A7B8E"] {
        let answer = card(&world.app, None, missing).await;
        assert_eq!(answer.status, StatusCode::NOT_FOUND, "{missing}");
        assert_eq!(answer.cache.as_deref(), Some("no-store"), "{missing}");
    }
}

#[tokio::test]
async fn the_card_names_both_players_once_the_room_is_full() {
    let world = world().await;
    let table = world.table("d3").await;
    let joiner = card(&world.app, Some(&table.joiner), &table.room).await;
    assert_eq!(joiner.json["you"], "O");
    assert_eq!(
        (&joiner.json["variant"], &joiner.json["rules"]),
        (&serde_json::json!("Deep"), &serde_json::json!("d3"))
    );
    let stranger = card(&world.app, None, &table.room).await;
    assert_eq!(stranger.json["you"], serde_json::Value::Null);
    assert_eq!(
        stranger.json["players"],
        serde_json::to_value(&table.players).unwrap()
    );
}

#[tokio::test]
async fn a_room_link_names_its_game_escaped() {
    let world = world().await;
    let host = guest(&world.app).await;
    let room = room(&world.app, &host, "std", "X").await;
    sqlx::query(
        "UPDATE accounts SET handle = $2
         WHERE id = (SELECT account_id FROM sessions WHERE id = $1)",
    )
    .bind(session_id(&host))
    .bind(r#"<b>&"x'"#)
    .execute(&world.pool)
    .await
    .unwrap();

    let named = page(&world.app, &format!("/r/{room}")).await;
    let title = r#"<meta property="og:title" content="&lt;b&gt;&amp;&quot;x&#39; invites you to a game of Standard" />"#;
    assert!(named.contains(title), "{named}");
    assert!(named.find(title).unwrap() < named.find("</head>").unwrap());

    let plain = page(&world.app, "/r/00000000000000000000").await;
    assert!(
        plain.contains(r#"content="Join a game of Kokonoe""#),
        "{plain}"
    );
}

#[tokio::test]
async fn two_players_play_a_whole_game() {
    let world = world().await;
    let (host, joiner) = (guest(&world.app).await, guest(&world.app).await);
    let room = room(&world.app, &host, "std", "X").await;
    let (host_name, joiner_name) = (
        handle(&world.pool, &host).await,
        handle(&world.pool, &joiner).await,
    );

    let mut x = world.enter(&host, &room).await;
    let (you, record, players) = welcomed(&mut x).await;
    assert_eq!(you, Player::X);
    assert_eq!(record.ruleset, Preset::Standard.ruleset());
    assert!(record.moves.is_empty());
    assert_eq!(players.get(Player::O), None, "waiting for the joiner");

    let mut o = world.enter(&joiner, &room).await;
    for (socket, seat) in [(&mut x, Player::X), (&mut o, Player::O)] {
        let (you, _, players) = welcomed(socket).await;
        assert_eq!(you, seat);
        assert_eq!(players.get(Player::X), Some(&*host_name));
        assert_eq!(players.get(Player::O), Some(&*joiner_name));
    }

    let mut table = Table {
        room,
        host,
        joiner,
        x,
        o,
        players: Players::default(),
    };
    let cells = cells(TOP_ROW);
    let (last, rest) = cells.split_last().unwrap();
    play(&mut table, 0, rest).await;
    let ply = rest.len() as u16;
    step(&mut table.x, ply, *last).await;
    let ended = ServerMsg::Ended(GameEnd {
        result: GameResult::XWins,
        termination: Termination::Line,
    });
    for socket in [&mut table.x, &mut table.o] {
        assert_eq!(hear(socket).await, moved(ply, *last));
        assert_eq!(hear(socket).await, ended);
    }
    let (moves, result): (Vec<i32>, Option<String>) =
        sqlx::query_as("SELECT moves, result FROM games WHERE room = $1")
            .bind(&table.room)
            .fetch_one(&world.pool)
            .await
            .unwrap();
    assert_eq!(moves.len(), cells.len());
    assert_eq!(result.as_deref(), Some("1-0"));

    step(&mut table.o, ply + 1, 0).await;
    assert_eq!(hear(&mut table.o).await, ended, "a move after the end");
}

#[tokio::test]
async fn a_deep_room_plays_its_cells_at_depth_3() {
    let world = world().await;
    let mut table = world.table("d3").await;
    let cells = cells(DEEP_OPENING);
    assert!(cells.iter().any(|&cell| cell > 80), "{cells:?}");
    play(&mut table, 0, &cells).await;
    refuse(&mut table, &[(Player::O, 3, 729, Rejection::IllegalMove)]).await;

    let mut again = world.enter(&table.host, &table.room).await;
    let (_, record, _) = welcomed(&mut again).await;
    assert_eq!(record.ruleset.depth, Depth::Three);
    assert_eq!(indices(&record), cells);
}

#[tokio::test]
async fn a_refused_move_changes_nothing() {
    let world = world().await;
    let (host, joiner) = (guest(&world.app).await, guest(&world.app).await);
    let room = room(&world.app, &host, "std", "X").await;
    let mut x = world.enter(&host, &room).await;
    welcomed(&mut x).await;
    step(&mut x, 0, 40).await;
    assert_eq!(
        hear(&mut x).await,
        rejected(Rejection::NotAllowed),
        "before the joiner"
    );

    let mut o = world.enter(&joiner, &room).await;
    welcomed(&mut x).await;
    welcomed(&mut o).await;
    let mut table = Table {
        room,
        host,
        joiner,
        x,
        o,
        players: Players::default(),
    };
    refuse(
        &mut table,
        &[
            (Player::X, 0, 81, Rejection::IllegalMove),
            (Player::O, 0, 40, Rejection::NotAllowed),
            (Player::X, 2, 40, Rejection::OutOfOrder { want: 0 }),
        ],
    )
    .await;
    play(&mut table, 0, &[40]).await;
    refuse(
        &mut table,
        &[
            (Player::X, 1, 36, Rejection::NotAllowed),
            (Player::O, 1, 40, Rejection::IllegalMove),
            (Player::O, 1, 0, Rejection::IllegalMove),
        ],
    )
    .await;

    assert_eq!(stored(&world.pool, &table.room).await, [40]);
    let mut again = world.enter(&table.host, &table.room).await;
    let (_, record, _) = welcomed(&mut again).await;
    assert_eq!(indices(&record), [40]);
}

#[tokio::test]
async fn a_resent_move_says_nothing_new() {
    let world = world().await;
    let mut table = world.table("std").await;
    play(&mut table, 0, &[40]).await;
    step(&mut table.x, 0, 40).await;
    play(&mut table, 1, &[36]).await;
    assert_eq!(stored(&world.pool, &table.room).await, [40, 36]);
}

#[tokio::test]
async fn hello_checks_the_build_and_the_room() {
    let world = world().await;
    let table = world.table("std").await;
    let stranger = guest(&world.app).await;

    let builds = [(PROTOCOL + 1, RULES_VERSION), (PROTOCOL, RULES_VERSION + 1)];
    for (protocol, rules_version) in builds {
        let mut socket = open(world.server.address, &stranger).await;
        let hello = ClientMsg::Hello {
            protocol,
            rules_version,
            room: table.room.clone(),
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
        let mut socket = world.enter(&stranger, missing).await;
        assert_eq!(closed(&mut socket).await, 4004, "{missing}");
    }
    let mut third = world.enter(&stranger, &table.room).await;
    assert_eq!(closed(&mut third).await, 4003, "full");

    let host = guest(&world.app).await;
    let other = room(&world.app, &host, "std", "X").await;
    let mut twice = world.enter(&host, &other).await;
    welcomed(&mut twice).await;
    say(&mut twice, &hello(&other)).await;
    assert_eq!(closed(&mut twice).await, 4003, "a second Hello");

    let mut first = open(world.server.address, &host).await;
    step(&mut first, 0, 40).await;
    assert_eq!(closed(&mut first).await, 1008, "a move before Hello");
}

#[tokio::test]
async fn a_frame_that_is_not_a_message_closes_the_socket() {
    let world = world().await;
    let mut table = world.table("std").await;
    table.x.send(Message::text("40 please")).await.unwrap();
    assert_eq!(closed(&mut table.x).await, 1007, "not JSON");
    let unknown = r#"{"type":"Claim"}"#;
    table.o.send(Message::text(unknown)).await.unwrap();
    assert_eq!(closed(&mut table.o).await, 1007, "an unknown type");

    let mut binary = open(world.server.address, &table.host).await;
    binary.send(Message::binary(vec![0; 4])).await.unwrap();
    assert_eq!(closed(&mut binary).await, 1003, "binary before Hello");
    let mut binary = world.enter(&table.host, &table.room).await;
    welcomed(&mut binary).await;
    binary.send(Message::binary(vec![0; 4])).await.unwrap();
    assert_eq!(closed(&mut binary).await, 1003, "binary after Hello");

    let mut huge = world.enter(&table.joiner, &table.room).await;
    welcomed(&mut huge).await;
    huge.send(Message::text("x".repeat(8192))).await.unwrap();
    assert_eq!(closed(&mut huge).await, 1002, "too big");
}

#[tokio::test]
async fn a_socket_that_never_says_hello_is_closed() {
    let world = world().await;
    let token = guest(&world.app).await;
    let mut silent = open(world.server.address, &token).await;
    let close = tokio::time::timeout(Duration::from_secs(15), async {
        loop {
            if let Some(Ok(Message::Close(Some(close)))) = silent.next().await {
                break u16::from(close.code);
            }
        }
    });
    assert_eq!(close.await.expect("closed within 15 s"), 1008);
}

#[tokio::test]
async fn a_reconnect_gets_a_fresh_welcome_and_plays_on() {
    let world = world().await;
    let mut table = world.table("std").await;
    let cells = cells(TOP_ROW);
    play(&mut table, 0, &cells[..3]).await;

    table.o = world.enter(&table.joiner, &table.room).await;
    let (you, record, players) = welcomed(&mut table.o).await;
    assert_eq!(you, Player::O);
    assert_eq!(indices(&record), cells[..3]);
    assert_eq!(players, table.players);
    play(&mut table, 3, &cells[3..5]).await;
}

#[tokio::test]
async fn a_room_no_one_is_in_is_loaded_again() {
    let world = world().await;
    let Table {
        room, host, x, o, ..
    } = world.table("std").await;
    drop((x, o));
    let cells = cells(TOP_ROW);
    // Behind the Room's back: only a Room loaded again would show this move.
    sqlx::query("UPDATE games SET moves = $2 WHERE room = $1")
        .bind(&room)
        .bind(vec![i32::from(cells[0])])
        .execute(&world.pool)
        .await
        .unwrap();

    let reloaded = async {
        loop {
            let mut x = world.enter(&host, &room).await;
            let (_, record, _) = welcomed(&mut x).await;
            if indices(&record) == cells[..1] {
                break;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    };
    tokio::time::timeout(Duration::from_secs(5), reloaded)
        .await
        .expect("the empty Room stopped, and was loaded from Postgres again");
}

#[tokio::test]
async fn a_restart_rebuilds_every_room_from_postgres() {
    let mut world = world().await;
    let mut table = world.table("std").await;
    let cells = cells(TOP_ROW);
    play(&mut table, 0, &cells[..8]).await;

    world.server.kill();
    world.server = Server::start(&world.url).await;

    table.x = world.enter(&table.host, &table.room).await;
    table.o = world.enter(&table.joiner, &table.room).await;
    for (socket, seat) in [(&mut table.x, Player::X), (&mut table.o, Player::O)] {
        let (you, record, players) = welcomed(socket).await;
        assert_eq!(you, seat);
        assert_eq!(players, table.players);
        assert_eq!(indices(&record), cells[..8]);
    }
    let (last, rest) = cells.split_last().unwrap();
    play(&mut table, 8, &rest[8..]).await;
    let ply = rest.len() as u16;
    step(&mut table.x, ply, *last).await;
    assert_eq!(hear(&mut table.o).await, moved(ply, *last));
    assert!(matches!(hear(&mut table.o).await, ServerMsg::Ended { .. }));
}

#[tokio::test]
async fn a_move_that_cannot_be_written_is_never_acknowledged() {
    let world = world().await;
    let mut table = world.table("std").await;
    let cells = cells(TOP_ROW);
    play(&mut table, 0, &cells[..2]).await;

    sqlx::query("ALTER TABLE games ADD CONSTRAINT short CHECK (cardinality(moves) < 3)")
        .execute(&world.pool)
        .await
        .unwrap();
    step(&mut table.x, 2, cells[2]).await;
    assert_eq!(closed(&mut table.x).await, 1011);
    assert_eq!(closed(&mut table.o).await, 1011);
    assert_eq!(stored(&world.pool, &table.room).await.len(), 2);

    sqlx::query("ALTER TABLE games DROP CONSTRAINT short")
        .execute(&world.pool)
        .await
        .unwrap();
    let mut x = world.enter(&table.host, &table.room).await;
    let (_, record, _) = welcomed(&mut x).await;
    assert_eq!(record.moves.len(), 2, "rebuilt from what was stored");
    step(&mut x, 2, cells[2]).await;
    assert_eq!(hear(&mut x).await, moved(2, cells[2]));
}
