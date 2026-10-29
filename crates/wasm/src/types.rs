//! The values that cross into JavaScript, as Rust mirrors of their TypeScript shapes.
//!
//! tsify writes these into the generated `.d.ts`, so they are exactly what the browser sees:
//! serde's default enum representation, a Cell as its base-9 index, and a Board as its Space path
//! from the Global board (`[]` is the Global board). A Ruleset and a Record follow the JSON of
//! spec §4.2 and §4.3.
//!
//! [`crate::convert`] maps each one to and from the engine with exhaustive matches, so a new
//! engine variant doesn't compile until it is declared here too. [`crate::js`] turns them into
//! plain JavaScript values and back, by hand, so no serializer ships in the module.

use tsify::Tsify;

#[derive(Tsify, Clone, Copy, Debug, PartialEq, Eq)]
pub enum Player {
    X,
    O,
}

/// A Cell as its base-9 index: 0 to 80 at depth 2 and 0 to 728 at depth 3, the first Space being
/// the most significant digit. The game supplies the depth.
#[derive(Tsify, Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cell(pub u16);

/// A Board as its Space path from the Global board down, each Space 0 to 8: `[]` is the Global
/// board and `[4]` is the middle Local board at depth 2.
#[derive(Tsify, Clone, Debug, PartialEq, Eq)]
pub struct Board(pub Vec<u8>);

/// How many marks are on the board, the dealt opening included.
#[derive(Tsify, Clone, Copy, Debug, PartialEq, Eq)]
pub struct Ply(pub u32);

/// A Seed as 16 lowercase hex digits, since a JSON number can't hold a u64 exactly in
/// JavaScript.
#[derive(Tsify, Clone, Debug, PartialEq, Eq)]
pub struct Seed(pub String);

/// The six rule settings, in spec §4.2's JSON. Standard is `{"depth":2,"continuation":"Off",
/// "drawn_owner":"Neither","objective":"Line","fallback":"Draw","opening":"Free"}`.
#[derive(Tsify, Clone, Copy, Debug, PartialEq, Eq)]
pub struct Ruleset {
    #[tsify(type = "2 | 3")]
    pub depth: u8,
    pub continuation: Continuation,
    pub drawn_owner: DrawnOwner,
    pub objective: Objective,
    pub fallback: Fallback,
    pub opening: Opening,
}

#[derive(Tsify, Clone, Copy, Debug, PartialEq, Eq)]
pub enum Continuation {
    Off,
    On,
}

#[derive(Tsify, Clone, Copy, Debug, PartialEq, Eq)]
pub enum DrawnOwner {
    Neither,
    Both,
}

/// `FirstTo` holds N, 1 to 9.
#[derive(Tsify, Clone, Copy, Debug, PartialEq, Eq)]
pub enum Objective {
    Line,
    FirstTo(u8),
}

#[derive(Tsify, Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fallback {
    Draw,
    MostWon,
}

/// `Random` holds k, the dealt plies, 1 to 8.
#[derive(Tsify, Clone, Copy, Debug, PartialEq, Eq)]
pub enum Opening {
    Free,
    Random(u8),
}

/// A game as data, in spec §4.3's JSON: what storage keeps, an export carries and a Worker
/// receives. `moves` excludes the dealt opening, which `seed` determines.
#[derive(Tsify, Clone, Debug, PartialEq, Eq)]
pub struct Record {
    pub rules_version: u32,
    pub ruleset: Ruleset,
    pub seed: Seed,
    pub moves: Vec<Cell>,
}

/// The game's progress. `moves` holds exactly the legal Cells, in ascending index order, and is
/// never empty.
#[derive(Tsify, Clone, Debug, PartialEq, Eq)]
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
#[derive(Tsify, Clone, Debug, PartialEq, Eq)]
pub enum Sent {
    FirstMove,
    To(Board),
    Free { target: Board, within: Board },
}

#[derive(Tsify, Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    Objective(Player),
    MostWon(Player),
    Draw,
}

#[derive(Tsify, Clone, Copy, Debug, PartialEq, Eq)]
pub enum BoardStatus {
    Open,
    Won(Player),
    Drawn,
}

/// A Cell the rules don't allow now. Each variant names the Board the UI flashes.
#[derive(Tsify, Clone, Debug, PartialEq, Eq)]
pub enum IllegalMove {
    GameOver,
    WrongDepth,
    Occupied,
    BeneathDecided { board: Board },
    OutsideActiveBoards { within: Board },
}

/// What `play` did: the move went in (`Now`), was already made at that ply (`Already`), or was
/// refused, as an illegal move or a ply out of order.
#[derive(Tsify, Clone, Debug, PartialEq, Eq)]
pub enum PlayResult {
    Now(Status),
    Already,
    Illegal(IllegalMove),
    OutOfOrder { want: Ply, got: Ply },
}

/// The first move of a Record that the rules reject, as an index into the Record's moves.
#[derive(Tsify, Clone, Debug, PartialEq, Eq)]
pub struct ReplayError {
    pub index: u32,
    pub error: IllegalMove,
}

/// One root move's visits from a search: how many of its iterations went through `cell`.
#[derive(Tsify, Clone, Copy, Debug, PartialEq, Eq)]
pub struct Visit {
    pub cell: Cell,
    pub count: u32,
}

/// What one search from a root found: a move the tactics force, or the visits of every root move
/// it could play, in ascending Cell index.
#[derive(Tsify, Clone, Debug, PartialEq, Eq)]
pub enum Searched {
    Forced(Cell),
    Visits(Vec<Visit>),
}

#[derive(Tsify, Clone, Debug, PartialEq, Eq)]
pub struct Preset {
    pub name: String,
    pub ruleset: Ruleset,
}
