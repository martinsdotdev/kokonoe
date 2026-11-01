//! The `games` table: one row a Room, written through before anything is acknowledged.

use kokonoe_engine::{Outcome, Player, Preset, RULES_VERSION, Record, Ruleset, Seed};
use kokonoe_protocol::Players;
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

/// A Room as stored: its game so far, and who sits where.
pub struct Stored {
    pub record: Record,
    pub seats: Seats,
}

/// A Room as its link describes it.
pub struct Summary {
    pub host: String,
    pub ruleset_code: String,
    pub seats: Seats,
}

impl Summary {
    /// The Variant's name: every Room plays a Preset for now.
    pub fn variant(&self) -> &'static str {
        self.ruleset_code
            .parse::<Ruleset>()
            .ok()
            .and_then(|ruleset| Preset::of(&ruleset))
            .map_or("Custom variant", Preset::name)
    }
}

/// Makes the Room `room`, with the host in `seat`, the other seat empty, and no moves.
pub async fn insert(
    pool: &PgPool,
    room: &RoomId,
    ruleset: Ruleset,
    seed: Seed,
    host: AccountId,
    seat: Player,
) -> Result<(), sqlx::Error> {
    let json = serde_json::to_string(&ruleset).expect("a Ruleset is always JSON");
    sqlx::query(
        "INSERT INTO games (room, rules_version, ruleset, ruleset_code, seed, host_id, x_id, o_id)
         VALUES ($1, $2, $3::jsonb, $4, $5, $6, $7, $8)",
    )
    .bind(room.as_str())
    .bind(i64::from(RULES_VERSION))
    .bind(json)
    .bind(ruleset.to_string())
    .bind(seed.to_string())
    .bind(host)
    .bind((seat == Player::X).then_some(host))
    .bind((seat == Player::O).then_some(host))
    .execute(pool)
    .await?;
    Ok(())
}

/// Puts `account` in the empty seat `player`; false if it wasn't empty, or `account` holds the
/// other one.
pub async fn seat(
    pool: &PgPool,
    room: &RoomId,
    player: Player,
    account: AccountId,
) -> Result<bool, sqlx::Error> {
    let query = match player {
        Player::X => {
            "UPDATE games SET x_id = $2, updated_at = now()
             WHERE room = $1 AND x_id IS NULL AND o_id IS DISTINCT FROM $2"
        }
        Player::O => {
            "UPDATE games SET o_id = $2, updated_at = now()
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

/// Appends `cell`, the move after the first `before`, and the Result when it ended the game.
/// False, changing nothing, unless the row holds exactly `before` moves and no Result, so a write
/// can never land twice or out of order.
pub async fn append(
    pool: &PgPool,
    room: &RoomId,
    before: usize,
    cell: u16,
    ended: Option<Outcome>,
) -> Result<bool, sqlx::Error> {
    let done = sqlx::query(
        "UPDATE games SET moves = array_append(moves, $3), result = $4, updated_at = now()
         WHERE room = $1 AND cardinality(moves) = $2 AND result IS NULL",
    )
    .bind(room.as_str())
    .bind(i64::try_from(before).expect("a game has at most 729 moves"))
    .bind(i32::from(cell))
    .bind(ended.map(result))
    .execute(pool)
    .await?;
    Ok(done.rows_affected() == 1)
}

/// Spec §4.4's Result for an ended game.
fn result(outcome: Outcome) -> &'static str {
    match outcome.winner() {
        Some(Player::X) => "1-0",
        Some(Player::O) => "0-1",
        None => "1/2-1/2",
    }
}

/// Both seats' columns, as `load` and `summary` select them.
#[derive(sqlx::FromRow)]
struct SeatColumns {
    x_id: Option<AccountId>,
    x: Option<String>,
    o_id: Option<AccountId>,
    o: Option<String>,
}

impl From<SeatColumns> for Seats {
    fn from(columns: SeatColumns) -> Seats {
        let seat = |account: Option<AccountId>, handle: Option<String>| {
            Some(Seat {
                account: account?,
                handle: handle?,
            })
        };
        Seats {
            x: seat(columns.x_id, columns.x),
            o: seat(columns.o_id, columns.o),
        }
    }
}

#[derive(sqlx::FromRow)]
struct StoredRow {
    record: String,
    #[sqlx(flatten)]
    seats: SeatColumns,
}

#[derive(sqlx::FromRow)]
struct SummaryRow {
    host: String,
    ruleset_code: String,
    #[sqlx(flatten)]
    seats: SeatColumns,
}

/// The Room `room`, as the server last stored it. Its Record is read through the engine's own
/// JSON, so it's checked as an import would be; one that doesn't read is an error.
pub async fn load(pool: &PgPool, room: &RoomId) -> Result<Option<Stored>, sqlx::Error> {
    let found: Option<StoredRow> = sqlx::query_as(
        "SELECT json_build_object('rules_version', g.rules_version, 'ruleset', g.ruleset,
                                  'seed', g.seed, 'moves', g.moves)::text AS record,
                g.x_id, x.handle::text AS x, g.o_id, o.handle::text AS o
         FROM games g
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
    Ok(Some(Stored {
        record,
        seats: found.seats.into(),
    }))
}

/// What the Room `room`'s link shows.
pub async fn summary(pool: &PgPool, room: &RoomId) -> Result<Option<Summary>, sqlx::Error> {
    let found: Option<SummaryRow> = sqlx::query_as(
        "SELECT h.handle::text AS host, g.ruleset_code,
                g.x_id, x.handle::text AS x, g.o_id, o.handle::text AS o
         FROM games g
         JOIN accounts h ON h.id = g.host_id
         LEFT JOIN accounts x ON x.id = g.x_id
         LEFT JOIN accounts o ON o.id = g.o_id
         WHERE g.room = $1",
    )
    .bind(room.as_str())
    .fetch_optional(pool)
    .await?;
    Ok(found.map(|found| Summary {
        host: found.host,
        ruleset_code: found.ruleset_code,
        seats: found.seats.into(),
    }))
}
