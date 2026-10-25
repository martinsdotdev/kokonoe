//! The engine's values as plain JavaScript values, and back, in the shapes [`crate::types`]
//! declares. Written by hand with `js_sys` rather than through serde, because serde-wasm-bindgen
//! alone cost about 30 KB brotli of the 100 KB game-screen budget.
//!
//! Building ([`ToJs`]) produces exactly the declared shape. Parsing (the `parse_*` functions) is
//! the boundary for data from JavaScript: it accepts exactly that shape, unknown fields refused,
//! or says what is [`Malformed`]. The browser tests in `web/src/lib/engine.browser.spec.ts` hold
//! both sides to the declarations.

use core::fmt;

use js_sys::{Array, Object, Reflect};
use kokonoe_engine as engine;
use wasm_bindgen::{JsCast, JsValue};

/// Data the engine can't even consider: wrong in shape, before any rule is consulted.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Malformed {
    /// Not the declared shape. Names what was expected.
    Shape(&'static str),
    /// A value outside the engine's range: a depth, a Space, a Goal, a Random k or a Cell index.
    Parse(engine::ParseError),
    /// A Board path that doesn't name a Board of the game: a Cell's path or longer.
    BoardPath(usize),
    /// A Record from another rules version than this build plays.
    RulesVersion(u32),
}

impl From<engine::ParseError> for Malformed {
    fn from(error: engine::ParseError) -> Malformed {
        Malformed::Parse(error)
    }
}

impl fmt::Display for Malformed {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Malformed::Shape(expected) => f.write_str(expected),
            Malformed::Parse(error) => write!(f, "{error}"),
            Malformed::BoardPath(n) => write!(f, "a Board path of {n} Spaces names no Board"),
            Malformed::RulesVersion(n) => write!(
                f,
                "the Record is from rules version {n}, and this build plays version {}",
                engine::RULES_VERSION
            ),
        }
    }
}

impl std::error::Error for Malformed {}

/// An engine value as the plain JavaScript value its declaration in [`crate::types`] describes.
pub trait ToJs {
    fn to_js(&self) -> JsValue;
}

fn object<const N: usize>(fields: [(&str, JsValue); N]) -> JsValue {
    let object = Object::new();
    for (key, value) in fields {
        // Defining a property on a fresh plain object can't fail.
        let _ = Reflect::set(&object, &JsValue::from_str(key), &value);
    }
    object.into()
}

/// A variant with data, tagged the serde way: `{ Variant: data }`. A variant without data is
/// just its name.
fn tagged(tag: &str, data: JsValue) -> JsValue {
    object([(tag, data)])
}

fn array(items: impl Iterator<Item = JsValue>) -> JsValue {
    items.collect::<Array>().into()
}

fn player_name(player: engine::Player) -> &'static str {
    match player {
        engine::Player::X => "X",
        engine::Player::O => "O",
    }
}

fn continuation_name(continuation: engine::Continuation) -> &'static str {
    match continuation {
        engine::Continuation::Off => "Off",
        engine::Continuation::On => "On",
    }
}

fn drawn_owner_name(owner: engine::DrawnOwner) -> &'static str {
    match owner {
        engine::DrawnOwner::Neither => "Neither",
        engine::DrawnOwner::Both => "Both",
    }
}

fn fallback_name(fallback: engine::Fallback) -> &'static str {
    match fallback {
        engine::Fallback::Draw => "Draw",
        engine::Fallback::MostWon => "MostWon",
    }
}

impl ToJs for engine::Player {
    fn to_js(&self) -> JsValue {
        player_name(*self).into()
    }
}

impl ToJs for engine::Cell {
    fn to_js(&self) -> JsValue {
        self.index().into()
    }
}

impl ToJs for engine::Board {
    fn to_js(&self) -> JsValue {
        array(self.spaces().iter().map(|space| space.index().into()))
    }
}

impl ToJs for engine::Ply {
    fn to_js(&self) -> JsValue {
        self.get().into()
    }
}

impl ToJs for [engine::Cell] {
    fn to_js(&self) -> JsValue {
        array(self.iter().map(ToJs::to_js))
    }
}

impl ToJs for engine::Status {
    fn to_js(&self) -> JsValue {
        match self {
            engine::Status::ToMove {
                player,
                sent,
                moves,
            } => tagged(
                "ToMove",
                object([
                    ("player", player.to_js()),
                    ("sent", sent.to_js()),
                    ("moves", array(moves.iter().map(|cell| cell.to_js()))),
                ]),
            ),
            engine::Status::Over(outcome) => tagged("Over", outcome.to_js()),
        }
    }
}

impl ToJs for engine::Sent {
    fn to_js(&self) -> JsValue {
        match self {
            engine::Sent::FirstMove => "FirstMove".into(),
            engine::Sent::To(board) => tagged("To", board.to_js()),
            engine::Sent::Free { target, within } => tagged(
                "Free",
                object([("target", target.to_js()), ("within", within.to_js())]),
            ),
        }
    }
}

impl ToJs for engine::Outcome {
    fn to_js(&self) -> JsValue {
        match self {
            engine::Outcome::Objective(player) => tagged("Objective", player.to_js()),
            engine::Outcome::MostWon(player) => tagged("MostWon", player.to_js()),
            engine::Outcome::Draw => "Draw".into(),
        }
    }
}

impl ToJs for engine::BoardStatus {
    fn to_js(&self) -> JsValue {
        match self {
            engine::BoardStatus::Open => "Open".into(),
            engine::BoardStatus::Won(player) => tagged("Won", player.to_js()),
            engine::BoardStatus::Drawn => "Drawn".into(),
        }
    }
}

impl ToJs for engine::IllegalMove {
    fn to_js(&self) -> JsValue {
        match self {
            engine::IllegalMove::GameOver => "GameOver".into(),
            engine::IllegalMove::WrongDepth => "WrongDepth".into(),
            engine::IllegalMove::Occupied => "Occupied".into(),
            engine::IllegalMove::BeneathDecided { board } => {
                tagged("BeneathDecided", object([("board", board.to_js())]))
            }
            engine::IllegalMove::OutsideActiveBoards { within } => {
                tagged("OutsideActiveBoards", object([("within", within.to_js())]))
            }
        }
    }
}

/// `play`'s answer as one PlayResult.
impl ToJs for Result<engine::Played, engine::PlayError> {
    fn to_js(&self) -> JsValue {
        match self {
            Ok(engine::Played::Now(status)) => tagged("Now", status.to_js()),
            Ok(engine::Played::Already) => "Already".into(),
            Err(engine::PlayError::Illegal(error)) => tagged("Illegal", error.to_js()),
            Err(engine::PlayError::OutOfOrder { want, got }) => tagged(
                "OutOfOrder",
                object([("want", want.to_js()), ("got", got.to_js())]),
            ),
        }
    }
}

impl ToJs for engine::ReplayError {
    fn to_js(&self) -> JsValue {
        object([
            ("index", JsValue::from(self.index)),
            ("error", self.error.to_js()),
        ])
    }
}

impl ToJs for engine::Ruleset {
    fn to_js(&self) -> JsValue {
        object([
            ("depth", u8::from(self.depth).into()),
            ("continuation", continuation_name(self.continuation).into()),
            ("drawn_owner", drawn_owner_name(self.drawn_owner).into()),
            (
                "objective",
                match self.objective {
                    engine::Objective::Line => "Line".into(),
                    engine::Objective::FirstTo(goal) => tagged("FirstTo", goal.get().into()),
                },
            ),
            ("fallback", fallback_name(self.fallback).into()),
            (
                "opening",
                match self.opening {
                    engine::Opening::Free => "Free".into(),
                    engine::Opening::Random(k) => tagged("Random", k.get().into()),
                },
            ),
        ])
    }
}

impl ToJs for engine::Record {
    fn to_js(&self) -> JsValue {
        object([
            ("rules_version", engine::RULES_VERSION.into()),
            ("ruleset", self.ruleset.to_js()),
            ("seed", seed_hex(self.seed).into()),
            ("moves", self.moves.to_js()),
        ])
    }
}

impl ToJs for engine::Preset {
    fn to_js(&self) -> JsValue {
        object([
            ("name", self.name().into()),
            ("ruleset", self.ruleset().to_js()),
        ])
    }
}

/// 16 lowercase hex digits.
pub fn seed_hex(seed: engine::Seed) -> String {
    let mut hex = String::with_capacity(16);
    for byte in seed.0.to_be_bytes() {
        for nibble in [byte >> 4, byte & 0xf] {
            hex.push(char::from_digit(u32::from(nibble), 16).unwrap_or('0'));
        }
    }
    hex
}

/// Exactly 16 hex digits, in either case.
pub fn seed_from_hex(hex: &str) -> Option<engine::Seed> {
    // from_str_radix alone would also take a sign.
    if hex.len() != 16 || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    u64::from_str_radix(hex, 16).ok().map(engine::Seed)
}

/// The Board at `path`, read from the Global board down.
pub fn board_at(path: &[u8], depth: engine::Depth) -> Result<engine::Board, Malformed> {
    let mut board = engine::Board::global(depth);
    for &space in path {
        board = match board.child(engine::Space::try_from(space)?) {
            engine::Child::Board(child) => child,
            engine::Child::Cell(_) => return Err(Malformed::BoardPath(path.len())),
        };
    }
    Ok(board)
}

fn is_plain_object(value: &JsValue) -> bool {
    value.is_object() && !Array::is_array(value)
}

/// The values of a plain object that has exactly `keys`, in their order.
fn fields<const N: usize>(
    value: &JsValue,
    keys: [&str; N],
    expected: &'static str,
) -> Result<[JsValue; N], Malformed> {
    let exact = is_plain_object(value)
        && usize::try_from(Object::keys(value.unchecked_ref::<Object>()).length()) == Ok(N);
    if !exact {
        return Err(Malformed::Shape(expected));
    }
    // Every key is required, so with the count right a missing one means an unknown one, and
    // reads as undefined, which no field accepts.
    Ok(keys.map(|key| Reflect::get(value, &JsValue::from_str(key)).unwrap_or(JsValue::UNDEFINED)))
}

/// The data of `{ tag: data }`, an object with that one key.
fn tagged_data(value: &JsValue, tag: &str) -> Option<JsValue> {
    let [data] = fields(value, [tag], "").ok()?;
    (!data.is_undefined()).then_some(data)
}

/// The variant of a data-free enum whose name `value` is.
fn parse_name<T: Copy>(
    value: &JsValue,
    all: &[T],
    name: fn(T) -> &'static str,
    expected: &'static str,
) -> Result<T, Malformed> {
    let given = value.as_string();
    all.iter()
        .copied()
        .find(|&variant| given.as_deref() == Some(name(variant)))
        .ok_or(Malformed::Shape(expected))
}

/// A whole number from 0 to `u32::MAX`.
fn whole(value: &JsValue) -> Option<u32> {
    let n = value.as_f64()?;
    (n >= 0.0 && n <= f64::from(u32::MAX) && n.trunc() == n).then_some(n as u32)
}

fn byte(value: &JsValue, expected: &'static str) -> Result<u8, Malformed> {
    whole(value)
        .and_then(|n| u8::try_from(n).ok())
        .ok_or(Malformed::Shape(expected))
}

pub fn parse_player(value: &JsValue) -> Result<engine::Player, Malformed> {
    parse_name(
        value,
        &[engine::Player::X, engine::Player::O],
        player_name,
        "a Player is \"X\" or \"O\"",
    )
}

pub fn parse_ply(value: &JsValue) -> Result<engine::Ply, Malformed> {
    whole(value)
        .and_then(|n| u16::try_from(n).ok())
        .map(engine::Ply::new)
        .ok_or(Malformed::Shape("a Ply is a whole number"))
}

pub fn parse_cell(value: &JsValue, depth: engine::Depth) -> Result<engine::Cell, Malformed> {
    let index = whole(value)
        .and_then(|n| u16::try_from(n).ok())
        .ok_or(Malformed::Shape("a Cell is its index, a whole number"))?;
    Ok(engine::Cell::from_index(depth, index)?)
}

pub fn parse_board(value: &JsValue, depth: engine::Depth) -> Result<engine::Board, Malformed> {
    const EXPECTED: &str = "a Board is its path of Spaces, an array of whole numbers";
    if !Array::is_array(value) {
        return Err(Malformed::Shape(EXPECTED));
    }
    let path = value
        .unchecked_ref::<Array>()
        .iter()
        .map(|space| byte(&space, EXPECTED))
        .collect::<Result<Vec<u8>, _>>()?;
    board_at(&path, depth)
}

pub fn parse_seed(value: &JsValue) -> Result<engine::Seed, Malformed> {
    value
        .as_string()
        .and_then(|hex| seed_from_hex(&hex))
        .ok_or(Malformed::Shape("a Seed is 16 hex digits"))
}

pub fn parse_ruleset(value: &JsValue) -> Result<engine::Ruleset, Malformed> {
    use engine::{Continuation, DrawnOwner, Fallback};
    let [
        depth,
        continuation,
        drawn_owner,
        objective,
        fallback,
        opening,
    ] = fields(
        value,
        [
            "depth",
            "continuation",
            "drawn_owner",
            "objective",
            "fallback",
            "opening",
        ],
        "a Ruleset is { depth, continuation, drawn_owner, objective, fallback, opening }",
    )?;
    let objective = if objective.as_string().as_deref() == Some("Line") {
        engine::Objective::Line
    } else {
        const EXPECTED: &str = "an objective is \"Line\" or { FirstTo: N }";
        let n = tagged_data(&objective, "FirstTo").ok_or(Malformed::Shape(EXPECTED))?;
        engine::Objective::FirstTo(engine::Goal::try_from(byte(&n, EXPECTED)?)?)
    };
    let opening = if opening.as_string().as_deref() == Some("Free") {
        engine::Opening::Free
    } else {
        const EXPECTED: &str = "an opening is \"Free\" or { Random: k }";
        let k = tagged_data(&opening, "Random").ok_or(Malformed::Shape(EXPECTED))?;
        engine::Opening::Random(engine::RandomPlies::try_from(byte(&k, EXPECTED)?)?)
    };
    Ok(engine::Ruleset {
        depth: engine::Depth::try_from(byte(&depth, "a depth is 2 or 3")?)?,
        continuation: parse_name(
            &continuation,
            &[Continuation::Off, Continuation::On],
            continuation_name,
            "continuation is \"Off\" or \"On\"",
        )?,
        drawn_owner: parse_name(
            &drawn_owner,
            &[DrawnOwner::Neither, DrawnOwner::Both],
            drawn_owner_name,
            "drawn_owner is \"Neither\" or \"Both\"",
        )?,
        objective,
        fallback: parse_name(
            &fallback,
            &[Fallback::Draw, Fallback::MostWon],
            fallback_name,
            "fallback is \"Draw\" or \"MostWon\"",
        )?,
        opening,
    })
}

pub fn parse_record(value: &JsValue) -> Result<engine::Record, Malformed> {
    let [rules_version, ruleset, seed, moves] = fields(
        value,
        ["rules_version", "ruleset", "seed", "moves"],
        "a Record is { rules_version, ruleset, seed, moves }",
    )?;
    let rules_version =
        whole(&rules_version).ok_or(Malformed::Shape("rules_version is a whole number"))?;
    if rules_version != engine::RULES_VERSION {
        return Err(Malformed::RulesVersion(rules_version));
    }
    let ruleset = parse_ruleset(&ruleset)?;
    if !Array::is_array(&moves) {
        return Err(Malformed::Shape("a Record's moves are an array of Cells"));
    }
    Ok(engine::Record {
        ruleset,
        seed: parse_seed(&seed)?,
        moves: moves
            .unchecked_ref::<Array>()
            .iter()
            .map(|cell| parse_cell(&cell, ruleset.depth))
            .collect::<Result<_, _>>()?,
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn seeds_are_16_hex_digits() {
        for n in [0, 0x5eed00, (1 << 53) + 1, u64::MAX] {
            let hex = seed_hex(engine::Seed(n));
            assert_eq!(hex.len(), 16);
            assert_eq!(seed_from_hex(&hex), Some(engine::Seed(n)));
        }
        assert_eq!(
            seed_hex(engine::Seed(0x5eed00)),
            "00000000005eed00",
            "spec §4.3's example"
        );
        assert_eq!(
            seed_from_hex("00000000005EED00"),
            Some(engine::Seed(0x5eed00))
        );
        for bad in [
            "5eed",
            "+000000000000000",
            "g000000000000000",
            "00000000000000000",
        ] {
            assert_eq!(seed_from_hex(bad), None, "{bad}");
        }
    }

    #[test]
    fn board_paths_name_boards_only() {
        for depth in [engine::Depth::Two, engine::Depth::Three] {
            let mut boards = vec![engine::Board::global(depth)];
            while let Some(board) = boards.pop() {
                let path: Vec<u8> = board.spaces().iter().map(|space| space.index()).collect();
                assert_eq!(board_at(&path, depth), Ok(board));
                for space in engine::Space::ALL {
                    if let engine::Child::Board(child) = board.child(space) {
                        boards.push(child);
                    }
                }
            }
        }
        assert_eq!(
            board_at(&[4, 2], engine::Depth::Two),
            Err(Malformed::BoardPath(2)),
            "a Cell's path"
        );
        assert_eq!(
            board_at(&[9], engine::Depth::Two),
            Err(Malformed::Parse(engine::ParseError::Space(9)))
        );
    }
}
