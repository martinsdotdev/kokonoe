//! A Room's policies (spec §10.2) over two sockets, against a real Postgres in Docker: resigning,
//! draw offers, takebacks and rematch. The test server's offers lapse sooner than a real one's
//! (`common::TIMING`).

mod common;

use std::time::Duration;

use common::table::*;
use common::{Server, Socket, TIMING, closed, guest, hear, say};
use kokonoe_engine::Player;
use kokonoe_protocol::{
    ClientMsg, Draws, GameEnd, GameResult, Offer, OfferKind, Rejection, ServerMsg, Termination,
};
use sqlx::PgPool;

/// The opening of `TOP_ROW`: X in Local board 0, O sent there and sending X to board 3.
const MOVES: &str = "03 30 04 40";

/// X takes Local boards 0, 1 and 2, each with a row: `common::table`'s `TOP_ROW`.
const TOP_ROW: &str = "03 30 04 40 05 51 13 31 14 41 15 52 26 62 27 72 28";

/// A Room of `rules` allowing takebacks, both seats filled, the host as X.
async fn takebacks(world: &World, rules: &str) -> Table {
    let body = format!(r#"{{"rules":"{rules}","seat":"X","takebacks":true}}"#);
    world.table_with(&body).await
}

/// `by`'s offer of `kind` as the players first hear it, with all of `common::TIMING`'s wait.
fn offer(kind: OfferKind, by: Player) -> ServerMsg {
    let wait = match kind {
        OfferKind::Draw => None,
        OfferKind::Takeback => Some(TIMING.takeback),
        OfferKind::Rematch => Some(TIMING.rematch),
    };
    let lapses_in = wait.map(|wait| u32::try_from(wait.as_millis()).unwrap());
    ServerMsg::Offer(Offer {
        kind,
        by,
        lapses_in,
    })
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

/// `player` says `message`, and both sockets hear `heard`, in order.
async fn both_hear(
    table: &mut Table,
    player: Player,
    message: ClientMsg,
    heard: impl IntoIterator<Item = ServerMsg>,
) {
    say(table.socket(player), &message).await;
    for expected in heard {
        assert_eq!(hear(&mut table.x).await, expected, "X, after {message:?}");
        assert_eq!(hear(&mut table.o).await, expected, "O, after {message:?}");
    }
}

/// `socket` says `message`, and is told it isn't allowed.
async fn not_allowed(socket: &mut Socket, message: ClientMsg) {
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

/// How many Rooms the table holds.
async fn rooms(pool: &PgPool) -> i64 {
    sqlx::query_scalar("SELECT count(*) FROM games")
        .fetch_one(pool)
        .await
        .unwrap()
}

/// `by` offers a rematch and the other player accepts: the new Room, as both sockets heard it.
async fn rematched(table: &mut Table, by: Player) -> String {
    let asked = offer(OfferKind::Rematch, by);
    both_hear(table, by, ClientMsg::OfferRematch, [asked]).await;
    let accept = ClientMsg::AnswerRematch { accept: true };
    say(table.socket(by.opponent()), &accept).await;
    let mut rooms = Vec::new();
    for socket in [&mut table.x, &mut table.o] {
        assert_eq!(hear(socket).await, answered(OfferKind::Rematch, true));
        match hear(socket).await {
            ServerMsg::RematchRoom { room } => rooms.push(room),
            other => panic!("not the rematch: {other:?}"),
        }
    }
    assert_eq!(rooms[0], rooms[1]);
    rooms.swap_remove(0)
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
    not_allowed(&mut table.x, ClientMsg::Resign).await;
    step(&mut table.x, 4, cells("33")[0]).await;
    assert_eq!(hear(&mut table.x).await, resigned, "a move after the end");

    let mut again = world.enter(&table.joiner, &table.room).await;
    let (_, snapshot) = welcome(&mut again).await;
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
async fn nothing_ends_before_the_second_seat_fills() {
    let world = world().await;
    let host = guest(&world.app).await;
    let room = room(&world.app, &host, "std", "X").await;
    let mut x = world.enter(&host, &room).await;
    welcome(&mut x).await;
    not_allowed(&mut x, ClientMsg::Resign).await;
    not_allowed(&mut x, ClientMsg::OfferDraw).await;
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
    not_allowed(&mut table.x, ClientMsg::OfferDraw).await;
    not_allowed(&mut table.x, ClientMsg::AnswerDraw { accept: true }).await;
    let no = ClientMsg::AnswerDraw { accept: false };
    both_hear(
        &mut table,
        Player::O,
        no,
        [answered(OfferKind::Draw, false)],
    )
    .await;

    both_hear(&mut table, Player::X, ClientMsg::OfferDraw, [offered]).await;
    // The other player's move leaves the offer standing.
    play(&mut table, 1, &cells(MOVES)[1..2]).await;
    let agreed = [
        answered(OfferKind::Draw, true),
        ended(GameResult::Draw, Termination::AgreedDraw),
    ];
    let yes = ClientMsg::AnswerDraw { accept: true };
    both_hear(&mut table, Player::O, yes, agreed).await;
    let stored = ending(&world.pool, &table.room).await;
    assert_eq!(
        stored,
        (Some("1/2-1/2".into()), Some("agreed draw".into()), 2, 0)
    );
}

#[tokio::test]
async fn a_draw_the_table_refuses_is_never_announced() {
    let world = world().await;
    let mut table = world.table("std").await;
    let offered = offer(OfferKind::Draw, Player::X);
    both_hear(&mut table, Player::X, ClientMsg::OfferDraw, [offered]).await;
    sqlx::query("ALTER TABLE games ADD CONSTRAINT no_draws CHECK (termination <> 'agreed draw')")
        .execute(&world.pool)
        .await
        .unwrap();
    say(&mut table.o, &ClientMsg::AnswerDraw { accept: true }).await;
    assert_eq!(closed(&mut table.x).await, 1011, "not told it was accepted");
    assert_eq!(closed(&mut table.o).await, 1011);
    let stored = ending(&world.pool, &table.room).await;
    assert_eq!((stored.0, stored.1), (None, None));
}

#[tokio::test]
async fn a_fourth_draw_offer_is_refused_even_after_a_restart() {
    let mut world = world().await;
    let mut table = world.table("std").await;
    play(&mut table, 0, &cells(MOVES)[..1]).await;
    for _ in 0..Draws::MAX {
        let offered = offer(OfferKind::Draw, Player::X);
        both_hear(&mut table, Player::X, ClientMsg::OfferDraw, [offered]).await;
        let no = ClientMsg::AnswerDraw { accept: false };
        both_hear(
            &mut table,
            Player::O,
            no,
            [answered(OfferKind::Draw, false)],
        )
        .await;
    }
    not_allowed(&mut table.x, ClientMsg::OfferDraw).await;

    // A new server loads the Room again from the table.
    world.server.kill();
    world.server = Server::start(&world.url).await;
    let mut x = world.enter(&table.host, &table.room).await;
    let (_, snapshot) = welcome(&mut x).await;
    assert_eq!(snapshot.draws, Draws { x: 3, o: 0 });
    not_allowed(&mut x, ClientMsg::OfferDraw).await;
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
    not_allowed(&mut table.o, ClientMsg::AnswerDraw { accept: true }).await;
}

#[tokio::test]
async fn an_ending_takes_a_waiting_offer_with_it() {
    let world = world().await;
    let mut table = world.table("std").await;
    play(&mut table, 0, &cells(MOVES)[..1]).await;
    let offered = offer(OfferKind::Draw, Player::X);
    both_hear(&mut table, Player::X, ClientMsg::OfferDraw, [offered]).await;
    let resigned = ended(GameResult::XWins, Termination::Resign);
    both_hear(&mut table, Player::O, ClientMsg::Resign, [resigned]).await;
    not_allowed(&mut table.o, ClientMsg::AnswerDraw { accept: true }).await;
    let mut again = world.enter(&table.joiner, &table.room).await;
    let (_, snapshot) = welcome(&mut again).await;
    assert_eq!(snapshot.offer, None);
}

#[tokio::test]
async fn only_the_other_player_answers_and_every_socket_hears() {
    let world = world().await;
    let mut table = world.table("std").await;
    let mut second_x = world.enter(&table.host, &table.room).await;
    welcome(&mut second_x).await;
    let mut second_o = world.enter(&table.joiner, &table.room).await;
    welcome(&mut second_o).await;

    let offered = offer(OfferKind::Draw, Player::X);
    both_hear(
        &mut table,
        Player::X,
        ClientMsg::OfferDraw,
        [offered.clone()],
    )
    .await;
    assert_eq!(hear(&mut second_x).await, offered);
    assert_eq!(hear(&mut second_o).await, offered);
    not_allowed(&mut second_x, ClientMsg::AnswerDraw { accept: true }).await;

    let no = ClientMsg::AnswerDraw { accept: false };
    say(&mut second_o, &no).await;
    let declined = answered(OfferKind::Draw, false);
    for socket in [&mut table.x, &mut table.o, &mut second_x, &mut second_o] {
        assert_eq!(hear(socket).await, declined);
    }
}

#[tokio::test]
async fn a_takeback_is_refused_where_the_room_doesnt_allow_it() {
    let world = world().await;
    let mut table = world.table("std").await;
    play(&mut table, 0, &cells(MOVES)[..1]).await;
    not_allowed(&mut table.x, ClientMsg::AskTakeback).await;
    let card = card(&world.app, None, &table.room).await;
    assert_eq!(card.json["takebacks"], false);
}

#[tokio::test]
async fn a_takeback_undoes_back_to_the_askers_last_move() {
    let world = world().await;
    let mut table = takebacks(&world, "std").await;
    let moves = cells(MOVES);
    play(&mut table, 0, &moves[..1]).await;
    not_allowed(&mut table.o, ClientMsg::AskTakeback).await;
    play(&mut table, 1, &moves[1..2]).await;
    not_allowed(&mut table.x, ClientMsg::AnswerTakeback { accept: true }).await;

    // X's last move was two plies ago: both come back.
    let asked = offer(OfferKind::Takeback, Player::X);
    both_hear(
        &mut table,
        Player::X,
        ClientMsg::AskTakeback,
        [asked.clone()],
    )
    .await;
    let taken = [
        answered(OfferKind::Takeback, true),
        ServerMsg::TookBack {
            to_ply: 0,
            clocks: None,
        },
    ];
    let yes = ClientMsg::AnswerTakeback { accept: true };
    both_hear(&mut table, Player::O, yes.clone(), taken.clone()).await;
    assert_eq!(stored(&world.pool, &table.room).await, Vec::<i32>::new());

    // Asked straight after X's own move: only that comes back.
    play(&mut table, 0, &moves[..1]).await;
    both_hear(&mut table, Player::X, ClientMsg::AskTakeback, [asked]).await;
    both_hear(&mut table, Player::O, yes, taken).await;
    let mut again = world.enter(&table.host, &table.room).await;
    let (_, record, _) = welcomed(&mut again).await;
    assert!(record.moves.is_empty(), "rebuilt from the table");
    play(&mut table, 0, &moves).await;
}

#[tokio::test]
async fn a_reconnect_hears_how_long_a_takeback_has_left() {
    let world = world().await;
    let mut table = takebacks(&world, "std").await;
    play(&mut table, 0, &cells(MOVES)[..1]).await;
    let asked = offer(OfferKind::Takeback, Player::X);
    both_hear(&mut table, Player::X, ClientMsg::AskTakeback, [asked]).await;
    let waited = Duration::from_millis(50);
    tokio::time::sleep(waited).await;

    let mut again = world.enter(&table.joiner, &table.room).await;
    let (_, snapshot) = welcome(&mut again).await;
    let offer = snapshot.offer.expect("the takeback still waits");
    let ms = |wait: Duration| u32::try_from(wait.as_millis()).unwrap();
    let left = offer.lapses_in.expect("a takeback lapses");
    let most = ms(TIMING.takeback - waited);
    assert!(left > 0 && left <= most, "{left} ms left, at most {most}");
    assert_eq!((offer.kind, offer.by), (OfferKind::Takeback, Player::X));
}

#[tokio::test]
async fn a_takeback_lapses_is_declined_or_is_withdrawn_by_a_move() {
    let world = world().await;
    let mut table = takebacks(&world, "std").await;
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
    let started = tokio::time::Instant::now();
    for socket in [&mut table.x, &mut table.o] {
        assert_eq!(hear(socket).await, refused, "it lapsed");
    }
    let waited = started.elapsed();
    assert!(
        waited >= TIMING.takeback - Duration::from_millis(50) && waited < TIMING.rematch,
        "it lapsed as a takeback does, after {waited:?}"
    );
    not_allowed(&mut table.o, ClientMsg::AnswerTakeback { accept: true }).await;

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
async fn a_rematch_swaps_the_seats_in_a_new_room_once() {
    let world = world().await;
    let mut table = takebacks(&world, "d3").await;
    let draw = offer(OfferKind::Draw, Player::X);
    both_hear(&mut table, Player::X, ClientMsg::OfferDraw, [draw]).await;
    let no = ClientMsg::AnswerDraw { accept: false };
    both_hear(
        &mut table,
        Player::O,
        no,
        [answered(OfferKind::Draw, false)],
    )
    .await;
    let resigned = ended(GameResult::XWins, Termination::Resign);
    both_hear(&mut table, Player::O, ClientMsg::Resign, [resigned]).await;

    // The player who resigned asks for the rematch.
    let rematch = rematched(&mut table, Player::O).await;
    let copied: (bool, bool, bool, bool, bool, bool, i32, i16, i16) = sqlx::query_as(
        "SELECT n.x_id = o.o_id, n.o_id = o.x_id, n.ruleset = o.ruleset,
                n.ruleset_code = 'd3', n.takebacks, n.seed <> o.seed, n.rules_version,
                n.x_draw_offers, n.o_draw_offers
         FROM games n JOIN games o ON o.room = $2 WHERE n.room = $1",
    )
    .bind(&rematch)
    .bind(&table.room)
    .fetch_one(&world.pool)
    .await
    .unwrap();
    assert_eq!(copied, (true, true, true, true, true, true, 1, 0, 0));

    not_allowed(&mut table.x, ClientMsg::OfferRematch).await;
    assert_eq!(rooms(&world.pool).await, 2, "one rematch a game");
    let mut again = world.enter(&table.host, &table.room).await;
    let (_, snapshot) = welcome(&mut again).await;
    assert_eq!(
        snapshot.rematch.as_deref(),
        Some(&*rematch),
        "a reconnect finds it"
    );

    let mut x = world.enter(&table.joiner, &rematch).await;
    let (you, record, players) = welcomed(&mut x).await;
    assert_eq!(you, Player::X, "the joiner plays X now");
    assert!(record.moves.is_empty());
    assert_eq!(players.get(Player::O), table.players.get(Player::X));
}

#[tokio::test]
async fn a_game_the_rules_ended_can_be_rematched() {
    let world = world().await;
    let mut table = world.table("std").await;
    let cells = cells(TOP_ROW);
    let (last, rest) = cells.split_last().unwrap();
    play(&mut table, 0, rest).await;
    let ply = rest.len() as u16;
    let won = [
        moved(ply, *last),
        ended(GameResult::XWins, Termination::Line),
    ];
    let last = ClientMsg::Move { ply, cell: *last };
    both_hear(&mut table, Player::X, last, won).await;
    let rematch = rematched(&mut table, Player::X).await;
    assert_eq!(rematch.len(), 20);
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
    let started = tokio::time::Instant::now();
    for socket in [&mut table.x, &mut table.o] {
        assert_eq!(hear(socket).await, refused, "it lapsed");
    }
    assert!(
        started.elapsed() >= TIMING.rematch - Duration::from_millis(50),
        "it lapsed as a rematch does"
    );
    not_allowed(&mut table.x, ClientMsg::AnswerRematch { accept: true }).await;
    both_hear(&mut table, Player::O, ClientMsg::OfferRematch, [asked]).await;
    let no = ClientMsg::AnswerRematch { accept: false };
    both_hear(&mut table, Player::X, no, [refused]).await;
    assert_eq!(rooms(&world.pool).await, 1, "no rematch was made");
}

#[tokio::test]
async fn offers_wait_their_turn() {
    let world = world().await;
    let mut table = takebacks(&world, "std").await;
    not_allowed(&mut table.x, ClientMsg::OfferRematch).await;
    not_allowed(&mut table.x, ClientMsg::AskTakeback).await;
    not_allowed(&mut table.o, ClientMsg::AnswerDraw { accept: true }).await;

    let offered = offer(OfferKind::Draw, Player::X);
    both_hear(&mut table, Player::X, ClientMsg::OfferDraw, [offered]).await;
    not_allowed(&mut table.o, ClientMsg::OfferDraw).await;
    not_allowed(&mut table.o, ClientMsg::AnswerTakeback { accept: true }).await;
    let mut late = world.enter(&table.joiner, &table.room).await;
    let (_, snapshot) = welcome(&mut late).await;
    let waiting = Offer {
        kind: OfferKind::Draw,
        by: Player::X,
        lapses_in: None,
    };
    assert_eq!(
        (snapshot.offer, snapshot.takebacks),
        (Some(waiting), true),
        "a reconnect sees the offer waiting"
    );

    let resigned = ended(GameResult::OWins, Termination::Resign);
    both_hear(&mut table, Player::X, ClientMsg::Resign, [resigned]).await;
    for message in [
        ClientMsg::OfferDraw,
        ClientMsg::AskTakeback,
        ClientMsg::Resign,
    ] {
        not_allowed(&mut table.o, message).await;
    }
}
