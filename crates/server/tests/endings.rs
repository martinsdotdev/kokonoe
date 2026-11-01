//! A Room's policies (spec §10.2) over two sockets, against a real Postgres in Docker: resigning,
//! draw offers, takebacks and rematch. The test server's offers lapse in 600 ms (`common::TIMING`).

mod common;

use common::table::*;
use common::{Server, hear, say};
use kokonoe_engine::Player;
use kokonoe_protocol::{
    ClientMsg, Draws, GameEnd, GameResult, Offer, OfferKind, Rejection, ServerMsg, Termination,
};
use sqlx::PgPool;

/// The opening of `TOP_ROW`: X in Local board 0, O sent there and sending X to board 3.
const MOVES: &str = "03 30 04 40";

/// A Room allowing takebacks, both seats filled, the host as X.
async fn takebacks(world: &World) -> Table {
    world
        .table_with(r#"{"rules":"std","seat":"X","takebacks":true}"#)
        .await
}

fn offer(kind: OfferKind, by: Player) -> ServerMsg {
    ServerMsg::Offer(Offer { kind, by })
}

fn answered(kind: OfferKind, accepted: bool) -> ServerMsg {
    ServerMsg::OfferAnswered { kind, accepted }
}

fn ended(result: GameResult, termination: Termination) -> ServerMsg {
    ServerMsg::Ended(GameEnd {
        result,
        termination,
    })
}

/// The socket of `player` at `table`.
fn socket(table: &mut Table, player: Player) -> &mut common::Socket {
    match player {
        Player::X => &mut table.x,
        Player::O => &mut table.o,
    }
}

/// `player` says `message`, and both sockets hear `heard`, in order.
async fn both_hear(
    table: &mut Table,
    player: Player,
    message: ClientMsg,
    heard: impl IntoIterator<Item = ServerMsg>,
) {
    say(socket(table, player), &message).await;
    for expected in heard {
        assert_eq!(hear(&mut table.x).await, expected, "X, after {message:?}");
        assert_eq!(hear(&mut table.o).await, expected, "O, after {message:?}");
    }
}

/// `player` says `message`, and is told it isn't allowed.
async fn not_allowed(table: &mut Table, player: Player, message: ClientMsg) {
    let socket = socket(table, player);
    say(socket, &message).await;
    let reason = Rejection::NotAllowed;
    assert_eq!(
        hear(socket).await,
        ServerMsg::Rejected { reason },
        "{message:?}"
    );
}

/// The Result, Termination and draw offers the table holds for `room`.
async fn ending(pool: &PgPool, room: &str) -> (Option<String>, Option<String>, i16, i16) {
    sqlx::query_as(
        "SELECT result, termination, x_draw_offers, o_draw_offers FROM games WHERE room = $1",
    )
    .bind(room)
    .fetch_one(pool)
    .await
    .unwrap()
}

#[tokio::test]
async fn resigning_ends_the_game_for_the_other_player() {
    let world = world().await;
    let mut table = world.table("std").await;
    play(&mut table, 0, &cells(MOVES)).await;
    let resigned = ended(GameResult::XWins, Termination::Resign);
    both_hear(&mut table, Player::O, ClientMsg::Resign, [resigned.clone()]).await;

    let stored = ending(&world.pool, &table.room).await;
    assert_eq!(
        (stored.0.as_deref(), stored.1.as_deref()),
        (Some("1-0"), Some("resign"))
    );
    not_allowed(&mut table, Player::X, ClientMsg::Resign).await;
    step(&mut table.x, 4, cells("33")[0]).await;
    assert_eq!(hear(&mut table.x).await, resigned, "a move after the end");

    let mut again = world.enter(&table.joiner, &table.room).await;
    let snapshot = match hear(&mut again).await {
        ServerMsg::Welcome { snapshot, .. } => snapshot,
        other => panic!("not a Welcome: {other:?}"),
    };
    assert_eq!(
        snapshot.end,
        Some(GameEnd {
            result: GameResult::XWins,
            termination: Termination::Resign
        }),
        "a reconnect is told how it ended"
    );
}

#[tokio::test]
async fn a_draw_is_offered_declined_and_agreed() {
    let world = world().await;
    let mut table = world.table("std").await;
    play(&mut table, 0, &cells(MOVES)[..1]).await;

    let offered = offer(OfferKind::Draw, Player::X);
    both_hear(
        &mut table,
        Player::X,
        ClientMsg::OfferDraw,
        [offered.clone()],
    )
    .await;
    not_allowed(&mut table, Player::X, ClientMsg::OfferDraw).await;
    not_allowed(
        &mut table,
        Player::X,
        ClientMsg::AnswerDraw { accept: true },
    )
    .await;
    let declined = answered(OfferKind::Draw, false);
    both_hear(
        &mut table,
        Player::O,
        ClientMsg::AnswerDraw { accept: false },
        [declined],
    )
    .await;

    both_hear(&mut table, Player::X, ClientMsg::OfferDraw, [offered]).await;
    // The other player's move leaves the offer standing.
    play(&mut table, 1, &cells(MOVES)[1..2]).await;
    let agreed = [
        answered(OfferKind::Draw, true),
        ended(GameResult::Draw, Termination::AgreedDraw),
    ];
    both_hear(
        &mut table,
        Player::O,
        ClientMsg::AnswerDraw { accept: true },
        agreed,
    )
    .await;
    let stored = ending(&world.pool, &table.room).await;
    assert_eq!(
        stored,
        (Some("1/2-1/2".into()), Some("agreed draw".into()), 2, 0)
    );
}

#[tokio::test]
async fn a_fourth_draw_offer_is_refused_even_after_a_restart() {
    let mut world = world().await;
    let mut table = world.table("std").await;
    play(&mut table, 0, &cells(MOVES)[..1]).await;
    let declined = answered(OfferKind::Draw, false);
    for _ in 0..3 {
        let offered = offer(OfferKind::Draw, Player::X);
        both_hear(&mut table, Player::X, ClientMsg::OfferDraw, [offered]).await;
        let no = ClientMsg::AnswerDraw { accept: false };
        both_hear(&mut table, Player::O, no, [declined.clone()]).await;
    }
    not_allowed(&mut table, Player::X, ClientMsg::OfferDraw).await;

    // A new server loads the Room again from the table.
    world.server.kill();
    world.server = Server::start(&world.url).await;
    let mut x = world.enter(&table.host, &table.room).await;
    let draws = match hear(&mut x).await {
        ServerMsg::Welcome { snapshot, .. } => snapshot.draws,
        other => panic!("not a Welcome: {other:?}"),
    };
    assert_eq!(draws, Draws { x: 3, o: 0 });
    say(&mut x, &ClientMsg::OfferDraw).await;
    let reason = Rejection::NotAllowed;
    assert_eq!(hear(&mut x).await, ServerMsg::Rejected { reason });
}

#[tokio::test]
async fn a_draw_offer_stands_until_its_player_moves() {
    let world = world().await;
    let mut table = world.table("std").await;
    let offered = offer(OfferKind::Draw, Player::X);
    both_hear(&mut table, Player::X, ClientMsg::OfferDraw, [offered]).await;
    let first = cells(MOVES)[0];
    say(
        &mut table.x,
        &ClientMsg::Move {
            ply: 0,
            cell: first,
        },
    )
    .await;
    for socket in [&mut table.x, &mut table.o] {
        assert_eq!(hear(socket).await, moved(0, first));
        assert_eq!(hear(socket).await, answered(OfferKind::Draw, false));
    }
    not_allowed(
        &mut table,
        Player::O,
        ClientMsg::AnswerDraw { accept: true },
    )
    .await;
}

#[tokio::test]
async fn a_takeback_is_refused_where_the_room_doesnt_allow_it() {
    let world = world().await;
    let mut table = world.table("std").await;
    play(&mut table, 0, &cells(MOVES)[..1]).await;
    not_allowed(&mut table, Player::X, ClientMsg::AskTakeback).await;
    let card = card(&world.app, None, &table.room).await;
    assert_eq!(card.json["takebacks"], false);
}

#[tokio::test]
async fn a_takeback_undoes_back_to_the_askers_last_move() {
    let world = world().await;
    let mut table = takebacks(&world).await;
    let moves = cells(MOVES);
    play(&mut table, 0, &moves[..2]).await;
    not_allowed(
        &mut table,
        Player::X,
        ClientMsg::AnswerTakeback { accept: true },
    )
    .await;

    // X's last move was two plies ago: both come back.
    let asked = offer(OfferKind::Takeback, Player::X);
    both_hear(&mut table, Player::X, ClientMsg::AskTakeback, [asked]).await;
    let taken = [
        answered(OfferKind::Takeback, true),
        ServerMsg::TookBack { to_ply: 0 },
    ];
    let yes = ClientMsg::AnswerTakeback { accept: true };
    both_hear(&mut table, Player::O, yes.clone(), taken.clone()).await;
    assert_eq!(stored(&world.pool, &table.room).await, Vec::<i32>::new());
    not_allowed(&mut table, Player::O, ClientMsg::AskTakeback).await;

    // Asked straight after X's own move: only that comes back.
    play(&mut table, 0, &moves[..1]).await;
    let asked = offer(OfferKind::Takeback, Player::X);
    both_hear(&mut table, Player::X, ClientMsg::AskTakeback, [asked]).await;
    both_hear(&mut table, Player::O, yes, taken).await;
    let mut again = world.enter(&table.host, &table.room).await;
    let (_, record, _) = welcomed(&mut again).await;
    assert!(record.moves.is_empty(), "rebuilt from the table");
    play(&mut table, 0, &moves).await;
}

#[tokio::test]
async fn a_takeback_lapses_is_declined_or_is_withdrawn_by_a_move() {
    let world = world().await;
    let mut table = takebacks(&world).await;
    let moves = cells(MOVES);
    play(&mut table, 0, &moves[..1]).await;
    let asked = offer(OfferKind::Takeback, Player::X);
    let refused = answered(OfferKind::Takeback, false);

    both_hear(
        &mut table,
        Player::X,
        ClientMsg::AskTakeback,
        [asked.clone()],
    )
    .await;
    for socket in [&mut table.x, &mut table.o] {
        assert_eq!(hear(socket).await, refused, "it lapsed");
    }
    both_hear(
        &mut table,
        Player::X,
        ClientMsg::AskTakeback,
        [asked.clone()],
    )
    .await;
    let no = ClientMsg::AnswerTakeback { accept: false };
    both_hear(&mut table, Player::O, no, [refused.clone()]).await;
    both_hear(&mut table, Player::X, ClientMsg::AskTakeback, [asked]).await;
    say(
        &mut table.o,
        &ClientMsg::Move {
            ply: 1,
            cell: moves[1],
        },
    )
    .await;
    for socket in [&mut table.x, &mut table.o] {
        assert_eq!(hear(socket).await, moved(1, moves[1]));
        assert_eq!(hear(socket).await, refused, "the move withdrew it");
    }
    assert_eq!(stored(&world.pool, &table.room).await.len(), 2);
}

#[tokio::test]
async fn a_rematch_swaps_the_seats_in_a_new_room() {
    let world = world().await;
    let mut table = takebacks(&world).await;
    let resigned = ended(GameResult::XWins, Termination::Resign);
    both_hear(&mut table, Player::O, ClientMsg::Resign, [resigned]).await;

    let asked = offer(OfferKind::Rematch, Player::X);
    both_hear(&mut table, Player::X, ClientMsg::OfferRematch, [asked]).await;
    say(&mut table.o, &ClientMsg::AnswerRematch { accept: true }).await;
    let mut rooms = Vec::new();
    for socket in [&mut table.x, &mut table.o] {
        assert_eq!(hear(socket).await, answered(OfferKind::Rematch, true));
        match hear(socket).await {
            ServerMsg::RematchRoom { room } => rooms.push(room),
            other => panic!("not the rematch: {other:?}"),
        }
    }
    assert_eq!(rooms[0], rooms[1]);
    let rematch = &rooms[0];

    let swapped: (bool, bool, bool, bool) = sqlx::query_as(
        "SELECT n.x_id = o.o_id, n.o_id = o.x_id, n.takebacks, n.result IS NULL
         FROM games n JOIN games o ON o.room = $2 WHERE n.room = $1",
    )
    .bind(rematch)
    .bind(&table.room)
    .fetch_one(&world.pool)
    .await
    .unwrap();
    assert_eq!(swapped, (true, true, true, true));
    let mut x = world.enter(&table.joiner, rematch).await;
    let (you, record, players) = welcomed(&mut x).await;
    assert_eq!(you, Player::X, "the joiner plays X now");
    assert!(record.moves.is_empty());
    assert_eq!(players.get(Player::O), table.players.get(Player::X));
}

#[tokio::test]
async fn a_rematch_lapses_or_is_declined() {
    let world = world().await;
    let mut table = world.table("std").await;
    let resigned = ended(GameResult::OWins, Termination::Resign);
    both_hear(&mut table, Player::X, ClientMsg::Resign, [resigned]).await;

    let asked = offer(OfferKind::Rematch, Player::O);
    let refused = answered(OfferKind::Rematch, false);
    both_hear(
        &mut table,
        Player::O,
        ClientMsg::OfferRematch,
        [asked.clone()],
    )
    .await;
    for socket in [&mut table.x, &mut table.o] {
        assert_eq!(hear(socket).await, refused, "it lapsed");
    }
    both_hear(&mut table, Player::O, ClientMsg::OfferRematch, [asked]).await;
    let no = ClientMsg::AnswerRematch { accept: false };
    both_hear(&mut table, Player::X, no, [refused]).await;
    let rooms: i64 = sqlx::query_scalar("SELECT count(*) FROM games")
        .fetch_one(&world.pool)
        .await
        .unwrap();
    assert_eq!(rooms, 1, "no rematch was made");
}

#[tokio::test]
async fn offers_wait_their_turn() {
    let world = world().await;
    let mut table = takebacks(&world).await;
    not_allowed(&mut table, Player::X, ClientMsg::OfferRematch).await;
    not_allowed(&mut table, Player::X, ClientMsg::AskTakeback).await;
    not_allowed(
        &mut table,
        Player::O,
        ClientMsg::AnswerDraw { accept: true },
    )
    .await;

    let offered = offer(OfferKind::Draw, Player::X);
    both_hear(
        &mut table,
        Player::X,
        ClientMsg::OfferDraw,
        [offered.clone()],
    )
    .await;
    not_allowed(&mut table, Player::O, ClientMsg::OfferDraw).await;
    not_allowed(
        &mut table,
        Player::O,
        ClientMsg::AnswerTakeback { accept: true },
    )
    .await;
    let mut late = world.enter(&table.joiner, &table.room).await;
    let snapshot = match hear(&mut late).await {
        ServerMsg::Welcome { snapshot, .. } => snapshot,
        other => panic!("not a Welcome: {other:?}"),
    };
    assert_eq!(
        (snapshot.offer, snapshot.takebacks),
        (
            Some(Offer {
                kind: OfferKind::Draw,
                by: Player::X
            }),
            true
        ),
        "a reconnect sees the offer waiting"
    );

    let resigned = ended(GameResult::OWins, Termination::Resign);
    both_hear(&mut table, Player::X, ClientMsg::Resign, [resigned]).await;
    for message in [
        ClientMsg::OfferDraw,
        ClientMsg::AskTakeback,
        ClientMsg::Resign,
    ] {
        not_allowed(&mut table, Player::O, message).await;
    }
}
