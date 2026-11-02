//! The `games` table: one row a Room, written through before anything is acknowledged.

use std::time::Duration;

use kokonoe_engine::{Player, Preset, RULES_VERSION, Record, Ruleset, Seed};
use kokonoe_protocol::{Draws, GameEnd, GameResult, Players, Termination, TimeControl};
use sqlx::PgPool;

use crate::room::RoomId;
use crate::session::AccountId;

/// Who holds a seat.
#[derive(Clone, Debug)]
pub struct Seat {
    pub account: AccountId,
    pub handle: String,
}

/// Both seats, each empty until someone takes it.
#[derive(Clone, Debug, Default)]
pub struct Seats {
    pub x: Option<Seat>,
    pub o: Option<Seat>,
}

impl Seats {
    pub fn get(&self, player: Player) -> Option<&Seat> {
        match player {
            Player::X => self.x.as_ref(),
            Player::O => self.o.as_ref(),
        }
    }

    pub fn set(&mut self, player: Player, seat: Seat) {
        match player {
            Player::X => self.x = Some(seat),
            Player::O => self.o = Some(seat),
        }
    }

    pub fn full(&self) -> bool {
        self.x.is_some() && self.o.is_some()
    }

    /// The first seat no one holds, if one is empty.
    pub fn empty(&self) -> Option<Player> {
        [Player::X, Player::O]
            .into_iter()
            .find(|&player| self.get(player).is_none())
    }

    /// The seat `account` holds, if it holds one.
    pub fn of(&self, account: AccountId) -> Option<Player> {
        [Player::X, Player::O]
            .into_iter()
            .find(|&player| self.get(player).is_some_and(|seat| seat.account == account))
    }

    pub fn players(&self) -> Players {
        let handle = |seat: &Option<Seat>| seat.as_ref().map(|seat| seat.handle.clone());
        Players {
            x: handle(&self.x),
            o: handle(&self.o),
        }
    }
}

/// A Room as stored: its game so far, its host's Handle, who sits where, the Room's options, the
/// draws offered, its clocks, and how the game ended once it has.
pub struct Stored {
    pub record: Record,
    pub host: String,
    pub seats: Seats,
    pub takebacks: bool,
    pub draws: Draws,
    /// Null for a game that goes on, or one that ended before Terminations were kept, which the
    /// rules can end again.
    pub end: Option<GameEnd>,
    /// The Room this game's rematch made, once one has.
    pub rematch: Option<RoomId>,
    /// The time control, or None for an untimed Room.
    pub clock: Option<TimeControl>,
    /// In a timed Room, each played move's mover's ms left after it.
    pub clocks: Vec<u32>,
    /// How long ago the turn now running began, by the table's clock.
    pub turn: Duration,
}

impl Stored {
    /// The Variant's name: every Room plays a Preset for now.
    pub fn variant(&self) -> &'static str {
        Preset::of(&self.record.ruleset).map_or("Custom variant", Preset::name)
    }
}

/// What a host makes a Room with.
pub struct NewGame {
    pub ruleset: Ruleset,
    pub seed: Seed,
    pub host: AccountId,
    pub seat: Player,
    pub takebacks: bool,
    pub clock: Option<TimeControl>,
}

/// Makes the Room `room`, with the host in their seat, the other seat empty, and no moves.
pub async fn insert(pool: &PgPool, room: &RoomId, game: NewGame) -> Result<(), sqlx::Error> {
    let NewGame {
        ruleset,
        seed,
        host,
        seat,
        takebacks,
        clock,
    } = game;
    let json = serde_json::to_string(&ruleset).expect("a Ruleset is always JSON");
    sqlx::query(
        "INSERT INTO games
             (room, rules_version, ruleset, ruleset_code, seed, host_id, x_id, o_id, takebacks,
              clock)
         VALUES ($1, $2, $3::jsonb, $4, $5, $6, $7, $8, $9, $10)",
    )
    .bind(room.as_str())
    .bind(i64::from(RULES_VERSION))
    .bind(json)
    .bind(ruleset.to_string())
    .bind(seed.to_string())
    .bind(host)
    .bind((seat == Player::X).then_some(host))
    .bind((seat == Player::O).then_some(host))
    .bind(takebacks)
    .bind(clock.map(|clock| clock.to_string()))
    .execute(pool)
    .await?;
    Ok(())
}

/// Puts `account` in the empty seat `player`; false if it wasn't empty, or `account` holds the
/// other one. Filling the second seat starts the first turn.
pub async fn seat(
    pool: &PgPool,
    room: &RoomId,
    player: Player,
    account: AccountId,
) -> Result<bool, sqlx::Error> {
    let query = match player {
        Player::X => {
            "UPDATE games SET x_id = $2, turn_since = now(), updated_at = now()
             WHERE room = $1 AND x_id IS NULL AND o_id IS DISTINCT FROM $2"
        }
        Player::O => {
            "UPDATE games SET o_id = $2, turn_since = now(), updated_at = now()
             WHERE room = $1 AND o_id IS NULL AND x_id IS DISTINCT FROM $2"
        }
    };
    let done = sqlx::query(query)
        .bind(room.as_str())
        .bind(account)
        .execute(pool)
        .await?;
    Ok(done.rows_affected() == 1)
}

/// A count of moves, as the table compares them.
fn count(moves: usize) -> i64 {
    i64::try_from(moves).expect("a game has at most 729 moves")
}

/// Appends `cell`, the move after the first `before`, with its mover's ms `left` in a timed Room,
/// and how it ended the game if it did. False, changing nothing, unless the row holds exactly
/// `before` moves and no Result, so a write can never land twice or out of order.
pub async fn append(
    pool: &PgPool,
    room: &RoomId,
    before: usize,
    cell: u16,
    left: Option<u32>,
    ended: Option<GameEnd>,
) -> Result<bool, sqlx::Error> {
    let done = sqlx::query(
        "UPDATE games SET moves = array_append(moves, $3), result = $4, termination = $5,
                          clocks = CASE WHEN $6::integer IS NULL THEN clocks
                                        ELSE array_append(clocks, $6) END,
                          turn_since = now(), updated_at = now()
         WHERE room = $1 AND cardinality(moves) = $2 AND result IS NULL",
    )
    .bind(room.as_str())
    .bind(count(before))
    .bind(i32::from(cell))
    .bind(ended.map(|end| end.result.as_str()))
    .bind(ended.map(|end| end.termination.as_str()))
    .bind(left.map(|left| i32::try_from(left).unwrap_or(i32::MAX)))
    .execute(pool)
    .await?;
    Ok(done.rows_affected() == 1)
}

/// Ends the game after its first `moves` moves without another: a resignation, an agreed draw or
/// a timeout.
pub async fn end(
    pool: &PgPool,
    room: &RoomId,
    moves: usize,
    end: GameEnd,
) -> Result<bool, sqlx::Error> {
    let done = sqlx::query(
        "UPDATE games SET result = $3, termination = $4, updated_at = now()
         WHERE room = $1 AND cardinality(moves) = $2 AND result IS NULL",
    )
    .bind(room.as_str())
    .bind(count(moves))
    .bind(end.result.as_str())
    .bind(end.termination.as_str())
    .execute(pool)
    .await?;
    Ok(done.rows_affected() == 1)
}

/// Takes the game, and its clocks, back from `before` moves to its first `to`: an accepted
/// takeback, which starts a turn.
pub async fn take_back(
    pool: &PgPool,
    room: &RoomId,
    before: usize,
    to: usize,
) -> Result<bool, sqlx::Error> {
    let done = sqlx::query(
        "UPDATE games SET moves = moves[1:$3], clocks = clocks[1:$3], turn_since = now(),
                          updated_at = now()
         WHERE room = $1 AND cardinality(moves) = $2 AND result IS NULL",
    )
    .bind(room.as_str())
    .bind(count(before))
    .bind(i32::try_from(to).expect("a game has at most 729 moves"))
    .execute(pool)
    .await?;
    Ok(done.rows_affected() == 1)
}

/// Deletes the game after its first `moves` moves, which goes on: an abort, which keeps nothing.
/// A rematch that named it names none.
pub async fn abort(pool: &PgPool, room: &RoomId, moves: usize) -> Result<bool, sqlx::Error> {
    let done = sqlx::query(
        "DELETE FROM games WHERE room = $1 AND cardinality(moves) = $2 AND result IS NULL",
    )
    .bind(room.as_str())
    .bind(count(moves))
    .execute(pool)
    .await?;
    Ok(done.rows_affected() == 1)
}

/// Counts one more draw offered by `player`, who had offered `made`.
pub async fn offer_draw(
    pool: &PgPool,
    room: &RoomId,
    player: Player,
    made: u8,
) -> Result<bool, sqlx::Error> {
    let query = match player {
        Player::X => {
            "UPDATE games SET x_draw_offers = x_draw_offers + 1
             WHERE room = $1 AND x_draw_offers = $2 AND result IS NULL"
        }
        Player::O => {
            "UPDATE games SET o_draw_offers = o_draw_offers + 1
             WHERE room = $1 AND o_draw_offers = $2 AND result IS NULL"
        }
    };
    let done = sqlx::query(query)
        .bind(room.as_str())
        .bind(i16::from(made))
        .execute(pool)
        .await?;
    Ok(done.rows_affected() == 1)
}

/// Makes `rematch` the next game of the finished Room `room`, which had none: the same rules and
/// options, a new Seed, and its players in each other's seats. The old row names it, in the same
/// statement, so a game is rematched once.
pub async fn rematch(
    pool: &PgPool,
    room: &RoomId,
    rematch: &RoomId,
    seed: Seed,
) -> Result<bool, sqlx::Error> {
    let done = sqlx::query(
        "WITH made AS (
             INSERT INTO games
                 (room, rules_version, ruleset, ruleset_code, seed, host_id, x_id, o_id, takebacks,
                  clock, turn_since)
             SELECT $2, $3, ruleset, ruleset_code, $4, host_id, o_id, x_id, takebacks, clock, now()
             FROM games WHERE room = $1 AND result IS NOT NULL AND rematch IS NULL
             RETURNING room
         )
         UPDATE games SET rematch = made.room, updated_at = now()
         FROM made WHERE games.room = $1",
    )
    .bind(room.as_str())
    .bind(rematch.as_str())
    .bind(i64::from(RULES_VERSION))
    .bind(seed.to_string())
    .execute(pool)
    .await?;
    Ok(done.rows_affected() == 1)
}

#[derive(sqlx::FromRow)]
struct StoredRow {
    record: String,
    host: String,
    x_id: Option<AccountId>,
    x: Option<String>,
    o_id: Option<AccountId>,
    o: Option<String>,
    takebacks: bool,
    x_draw_offers: i16,
    o_draw_offers: i16,
    result: Option<String>,
    termination: Option<String>,
    rematch: Option<String>,
    clock: Option<String>,
    clocks: Vec<i32>,
    turn_ms: i64,
}

/// The Room `room`, as the server last stored it. Its Record is read through the engine's own
/// JSON, so it's checked as an import would be; one that doesn't read is an error.
pub async fn load(pool: &PgPool, room: &RoomId) -> Result<Option<Stored>, sqlx::Error> {
    let found: Option<StoredRow> = sqlx::query_as(
        "SELECT json_build_object('rules_version', g.rules_version, 'ruleset', g.ruleset,
                                  'seed', g.seed, 'moves', g.moves)::text AS record,
                h.handle::text AS host, g.x_id, x.handle::text AS x, g.o_id, o.handle::text AS o,
                g.takebacks, g.x_draw_offers, g.o_draw_offers, g.result, g.termination, g.rematch,
                g.clock, g.clocks,
                COALESCE((extract(epoch FROM now() - g.turn_since) * 1000)::bigint, 0) AS turn_ms
         FROM games g
         JOIN accounts h ON h.id = g.host_id
         LEFT JOIN accounts x ON x.id = g.x_id
         LEFT JOIN accounts o ON o.id = g.o_id
         WHERE g.room = $1",
    )
    .bind(room.as_str())
    .fetch_optional(pool)
    .await?;
    let Some(found) = found else {
        return Ok(None);
    };
    let record =
        serde_json::from_str(&found.record).map_err(|error| sqlx::Error::Decode(error.into()))?;
    let seat = |account: Option<AccountId>, handle: Option<String>| {
        Some(Seat {
            account: account?,
            handle: handle?,
        })
    };
    let draws = |made: i16| u8::try_from(made).unwrap_or(Draws::MAX);
    let end = match (found.result.as_deref(), found.termination.as_deref()) {
        (Some(result), Some(termination)) => Some(GameEnd {
            result: GameResult::parse(result).ok_or_else(|| unreadable("a Result"))?,
            termination: Termination::parse(termination)
                .ok_or_else(|| unreadable("a Termination"))?,
        }),
        _ => None,
    };
    Ok(Some(Stored {
        record,
        host: found.host,
        seats: Seats {
            x: seat(found.x_id, found.x),
            o: seat(found.o_id, found.o),
        },
        takebacks: found.takebacks,
        draws: Draws {
            x: draws(found.x_draw_offers),
            o: draws(found.o_draw_offers),
        },
        end,
        rematch: match found.rematch {
            Some(rematch) => Some(RoomId::parse(&rematch).ok_or_else(|| unreadable("a Room id"))?),
            None => None,
        },
        clock: match found.clock {
            Some(clock) => Some(TimeControl::parse(&clock).ok_or_else(|| unreadable("a clock"))?),
            None => None,
        },
        clocks: found
            .clocks
            .into_iter()
            .map(|ms| u32::try_from(ms).map_err(|_| unreadable("a clock")))
            .collect::<Result<_, _>>()?,
        turn: Duration::from_millis(u64::try_from(found.turn_ms).unwrap_or(0)),
    }))
}

/// A column the table's own CHECKs should have kept from holding what it holds.
fn unreadable(what: &str) -> sqlx::Error {
    sqlx::Error::Decode(format!("the games table holds {what} this build can't read").into())
}
