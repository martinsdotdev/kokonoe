//! The TypeScript declarations of the values that cross into JavaScript.
//!
//! tsify writes these into the generated `.d.ts`, so they are exactly what the browser sees:
//! serde's default enum representation, a Cell as its base-9 index, and a Board as its Space path
//! from the Global board (`[]` is the Global board). A Ruleset and a Record follow the JSON of
//! spec §4.2 and §4.3. The values themselves are built and parsed by hand in [`crate::js`], so no
//! serializer ships in the module.
#![allow(
    dead_code,
    reason = "declarations for tsify: the values are built and read in JavaScript, see js.rs"
)]

use tsify::Tsify;

#[derive(Tsify)]
pub enum Player {
    X,
    O,
}

/// A Cell as its base-9 index: 0 to 80 at depth 2 and 0 to 728 at depth 3, the first Space being
/// the most significant digit. The game supplies the depth.
#[derive(Tsify)]
pub struct Cell(u16);

/// A Board as its Space path from the Global board down, each Space 0 to 8: `[]` is the Global
/// board and `[4]` is the middle Local board at depth 2.
#[derive(Tsify)]
pub struct Board(Vec<u8>);

/// How many marks are on the board, the dealt opening included.
#[derive(Tsify)]
pub struct Ply(u16);

/// A Seed as 16 hex digits, since a JSON number can't hold a u64 exactly in JavaScript.
#[derive(Tsify)]
pub struct Seed(String);

/// The six rule settings, in spec §4.2's JSON. Standard is `{"depth":2,"continuation":"Off",
/// "drawn_owner":"Neither","objective":"Line","fallback":"Draw","opening":"Free"}`.
#[derive(Tsify)]
pub struct Ruleset {
    #[tsify(type = "2 | 3")]
    depth: u8,
    continuation: Continuation,
    drawn_owner: DrawnOwner,
    objective: Objective,
    fallback: Fallback,
    opening: Opening,
}

#[derive(Tsify)]
pub enum Continuation {
    Off,
    On,
}

#[derive(Tsify)]
pub enum DrawnOwner {
    Neither,
    Both,
}

/// `FirstTo` holds N, 1 to 9.
#[derive(Tsify)]
pub enum Objective {
    Line,
    FirstTo(u8),
}

#[derive(Tsify)]
pub enum Fallback {
    Draw,
    MostWon,
}

/// `Random` holds k, the dealt plies, 1 to 8.
#[derive(Tsify)]
pub enum Opening {
    Free,
    Random(u8),
}

/// A game as data, in spec §4.3's JSON: what storage keeps, an export carries and a Worker
/// receives. `moves` excludes the dealt opening, which `seed` determines.
#[derive(Tsify)]
pub struct Record {
    rules_version: u32,
    ruleset: Ruleset,
    seed: Seed,
    moves: Vec<Cell>,
}

/// The game's progress. `moves` holds exactly the legal Cells, in ascending index order, and is
/// never empty.
#[derive(Tsify)]
pub enum Status {
    ToMove {
        player: Player,
        sent: Sent,
        moves: Vec<Cell>,
    },
    Over(Outcome),
}

/// The constraint the last move places on the side to move. Under `Free`, the Sent target can
/// take no move, so the player may play anywhere legal inside `within`.
#[derive(Tsify)]
pub enum Sent {
    FirstMove,
    To(Board),
    Free { target: Board, within: Board },
}

#[derive(Tsify)]
pub enum Outcome {
    Objective(Player),
    MostWon(Player),
    Draw,
}

#[derive(Tsify)]
pub enum BoardStatus {
    Open,
    Won(Player),
    Drawn,
}

/// A Cell the rules don't allow now. Each variant names the Board the UI flashes.
#[derive(Tsify)]
pub enum IllegalMove {
    GameOver,
    WrongDepth,
    Occupied,
    BeneathDecided { board: Board },
    OutsideActiveBoards { within: Board },
}

/// What `play` did: the move went in (`Now`), was already at that ply (`Already`), or was
/// refused, as an illegal move or a ply out of order.
#[derive(Tsify)]
pub enum PlayResult {
    Now(Status),
    Already,
    Illegal(IllegalMove),
    OutOfOrder { want: Ply, got: Ply },
}

/// The first move of a Record that the rules reject, as an index into the Record's moves.
#[derive(Tsify)]
pub struct ReplayError {
    index: u32,
    error: IllegalMove,
}

#[derive(Tsify)]
pub struct Preset {
    name: String,
    ruleset: Ruleset,
}
