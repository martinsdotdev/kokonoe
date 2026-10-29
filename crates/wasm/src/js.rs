//! The [`crate::types`] mirrors as plain JavaScript values, and back. Written by hand with
//! `js_sys` rather than through serde, because serde-wasm-bindgen alone cost about 20 KB brotli
//! of the 100 KB game-screen budget.
//!
//! Building ([`ToJs`]) matches every mirror exhaustively and produces exactly its declared
//! shape. Parsing (the `parse_*` functions) is the boundary for data from JavaScript. It reads
//! only own data properties, so prototypes and getters never supply a field, and unknown fields
//! are refused. Every JavaScript call it makes on such data catches exceptions: one thrown
//! through the WASM frames would skip Rust's destructors and leave the Game borrowed. The browser
//! tests in `web/src/lib/engine.browser.spec.ts` hold both directions to the declarations.

use js_sys::{Array, Object, Reflect};
use wasm_bindgen::prelude::*;

use crate::convert::Malformed;
use crate::types::{
    Board, BoardStatus, Cell, Continuation, DrawnOwner, Fallback, IllegalMove, Objective, Opening,
    Outcome, PlayResult, Player, Ply, Preset, Record, ReplayError, Ruleset, Searched, Seed, Sent,
    Status, Visit,
};

#[wasm_bindgen]
extern "C" {
    /// `Array.isArray`, imported to catch: it throws for a revoked Proxy.
    #[wasm_bindgen(catch, js_namespace = Array, js_name = isArray)]
    fn is_array(value: &JsValue) -> Result<bool, JsValue>;
}

/// A mirror as the plain JavaScript value its declaration describes.
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

impl ToJs for Player {
    fn to_js(&self) -> JsValue {
        match self {
            Player::X => "X".into(),
            Player::O => "O".into(),
        }
    }
}

impl ToJs for Cell {
    fn to_js(&self) -> JsValue {
        self.0.into()
    }
}

impl ToJs for [Cell] {
    fn to_js(&self) -> JsValue {
        array(self.iter().map(ToJs::to_js))
    }
}

impl ToJs for Board {
    fn to_js(&self) -> JsValue {
        array(self.0.iter().map(|&space| space.into()))
    }
}

impl ToJs for Ply {
    fn to_js(&self) -> JsValue {
        self.0.into()
    }
}

impl ToJs for Seed {
    fn to_js(&self) -> JsValue {
        self.0.as_str().into()
    }
}

impl ToJs for Ruleset {
    fn to_js(&self) -> JsValue {
        let continuation = match self.continuation {
            Continuation::Off => "Off",
            Continuation::On => "On",
        };
        let drawn_owner = match self.drawn_owner {
            DrawnOwner::Neither => "Neither",
            DrawnOwner::Both => "Both",
        };
        let objective = match self.objective {
            Objective::Line => "Line".into(),
            Objective::FirstTo(n) => tagged("FirstTo", n.into()),
        };
        let fallback = match self.fallback {
            Fallback::Draw => "Draw",
            Fallback::MostWon => "MostWon",
        };
        let opening = match self.opening {
            Opening::Free => "Free".into(),
            Opening::Random(k) => tagged("Random", k.into()),
        };
        object([
            ("depth", self.depth.into()),
            ("continuation", continuation.into()),
            ("drawn_owner", drawn_owner.into()),
            ("objective", objective),
            ("fallback", fallback.into()),
            ("opening", opening),
        ])
    }
}

impl ToJs for Record {
    fn to_js(&self) -> JsValue {
        object([
            ("rules_version", self.rules_version.into()),
            ("ruleset", self.ruleset.to_js()),
            ("seed", self.seed.to_js()),
            ("moves", self.moves.to_js()),
        ])
    }
}

impl ToJs for Status {
    fn to_js(&self) -> JsValue {
        match self {
            Status::ToMove {
                player,
                sent,
                moves,
            } => tagged(
                "ToMove",
                object([
                    ("player", player.to_js()),
                    ("sent", sent.to_js()),
                    ("moves", moves.to_js()),
                ]),
            ),
            Status::Over(outcome) => tagged("Over", outcome.to_js()),
        }
    }
}

impl ToJs for Sent {
    fn to_js(&self) -> JsValue {
        match self {
            Sent::FirstMove => "FirstMove".into(),
            Sent::To(board) => tagged("To", board.to_js()),
            Sent::Free { target, within } => tagged(
                "Free",
                object([("target", target.to_js()), ("within", within.to_js())]),
            ),
        }
    }
}

impl ToJs for Outcome {
    fn to_js(&self) -> JsValue {
        match self {
            Outcome::Objective(player) => tagged("Objective", player.to_js()),
            Outcome::MostWon(player) => tagged("MostWon", player.to_js()),
            Outcome::Draw => "Draw".into(),
        }
    }
}

impl ToJs for BoardStatus {
    fn to_js(&self) -> JsValue {
        match self {
            BoardStatus::Open => "Open".into(),
            BoardStatus::Won(player) => tagged("Won", player.to_js()),
            BoardStatus::Drawn => "Drawn".into(),
        }
    }
}

impl ToJs for IllegalMove {
    fn to_js(&self) -> JsValue {
        match self {
            IllegalMove::GameOver => "GameOver".into(),
            IllegalMove::WrongDepth => "WrongDepth".into(),
            IllegalMove::Occupied => "Occupied".into(),
            IllegalMove::BeneathDecided { board } => {
                tagged("BeneathDecided", object([("board", board.to_js())]))
            }
            IllegalMove::OutsideActiveBoards { within } => {
                tagged("OutsideActiveBoards", object([("within", within.to_js())]))
            }
        }
    }
}

impl ToJs for PlayResult {
    fn to_js(&self) -> JsValue {
        match self {
            PlayResult::Now(status) => tagged("Now", status.to_js()),
            PlayResult::Already => "Already".into(),
            PlayResult::Illegal(error) => tagged("Illegal", error.to_js()),
            PlayResult::OutOfOrder { want, got } => tagged(
                "OutOfOrder",
                object([("want", want.to_js()), ("got", got.to_js())]),
            ),
        }
    }
}

impl ToJs for ReplayError {
    fn to_js(&self) -> JsValue {
        object([("index", self.index.into()), ("error", self.error.to_js())])
    }
}

impl ToJs for Visit {
    fn to_js(&self) -> JsValue {
        object([("cell", self.cell.to_js()), ("visits", self.visits.into())])
    }
}

impl ToJs for Searched {
    fn to_js(&self) -> JsValue {
        match self {
            Searched::Forced(cell) => tagged("Forced", cell.to_js()),
            Searched::Visits(visits) => tagged("Visits", array(visits.iter().map(ToJs::to_js))),
        }
    }
}

impl ToJs for Preset {
    fn to_js(&self) -> JsValue {
        object([
            ("name", self.name.as_str().into()),
            ("ruleset", self.ruleset.to_js()),
        ])
    }
}

/// The value of `object`'s own data property `key`. `None` for a missing key, an accessor, an
/// inherited property, or an object that throws when asked.
fn own(object: &JsValue, key: &JsValue) -> Option<JsValue> {
    let descriptor =
        Reflect::get_own_property_descriptor(object.unchecked_ref::<Object>(), key).ok()?;
    if descriptor.is_undefined() {
        return None;
    }
    // A descriptor is a fresh plain object, even from a Proxy, so reading it runs no user code.
    let value = Reflect::get(&descriptor, &JsValue::from_str("value")).ok()?;
    (!value.is_undefined()).then_some(value)
}

fn is_plain_object(value: &JsValue) -> bool {
    value.is_object() && is_array(value) == Ok(false)
}

/// The values of a plain object whose own properties are exactly `keys`, in their order.
fn fields<const N: usize>(
    value: &JsValue,
    keys: [&str; N],
    expected: &'static str,
) -> Result<[JsValue; N], Malformed> {
    let count = is_plain_object(value)
        .then(|| Reflect::own_keys(value).ok())
        .flatten()
        .map(|own_keys| own_keys.length());
    if count.and_then(|n| usize::try_from(n).ok()) != Some(N) {
        return Err(Malformed::Shape(expected));
    }
    let values = keys.map(|key| own(value, &JsValue::from_str(key)));
    if values.iter().any(Option::is_none) {
        return Err(Malformed::Shape(expected));
    }
    Ok(values.map(|value| value.unwrap_or(JsValue::UNDEFINED)))
}

/// The elements of an array of at most `max`, all present.
fn elements(value: &JsValue, max: u32, expected: &'static str) -> Result<Vec<JsValue>, Malformed> {
    let length = (is_array(value) == Ok(true))
        .then(|| own(value, &JsValue::from_str("length")))
        .flatten()
        .and_then(|length| whole(&length))
        .filter(|&length| length <= max)
        .ok_or(Malformed::Shape(expected))?;
    (0..length)
        .map(|i| own(value, &i.into()).ok_or(Malformed::Shape(expected)))
        .collect()
}

/// The data of `{ tag: data }`, an object with that one own key.
fn tagged_data(value: &JsValue, tag: &str) -> Option<JsValue> {
    let [data] = fields(value, [tag], "").ok()?;
    Some(data)
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

fn name(value: &JsValue) -> Option<String> {
    value.as_string()
}

pub fn parse_player(value: &JsValue) -> Result<Player, Malformed> {
    match name(value).as_deref() {
        Some("X") => Ok(Player::X),
        Some("O") => Ok(Player::O),
        _ => Err(Malformed::Shape("a Player is \"X\" or \"O\"")),
    }
}

/// Any whole number: past the engine's range it is simply a ply out of order.
pub fn parse_ply(value: &JsValue) -> Result<Ply, Malformed> {
    whole(value)
        .map(Ply)
        .ok_or(Malformed::Shape("a Ply is a whole number"))
}

pub fn parse_cell(value: &JsValue) -> Result<Cell, Malformed> {
    whole(value)
        .and_then(|n| u16::try_from(n).ok())
        .map(Cell)
        .ok_or(Malformed::Shape("a Cell is its index, a whole number"))
}

pub fn parse_board(value: &JsValue) -> Result<Board, Malformed> {
    const EXPECTED: &str = "a Board is its path of Spaces, an array of whole numbers";
    // Three Spaces already name a Cell, which Board::at_depth reports.
    let path = elements(value, 3, EXPECTED)?;
    Ok(Board(
        path.iter()
            .map(|space| byte(space, EXPECTED))
            .collect::<Result<_, _>>()?,
    ))
}

pub fn parse_seed(value: &JsValue) -> Result<Seed, Malformed> {
    name(value)
        .map(Seed)
        .ok_or(Malformed::Shape("a Seed is 16 lowercase hex digits"))
}

pub fn parse_ruleset(value: &JsValue) -> Result<Ruleset, Malformed> {
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
    Ok(Ruleset {
        depth: byte(&depth, "a depth is 2 or 3")?,
        continuation: match name(&continuation).as_deref() {
            Some("Off") => Continuation::Off,
            Some("On") => Continuation::On,
            _ => return Err(Malformed::Shape("continuation is \"Off\" or \"On\"")),
        },
        drawn_owner: match name(&drawn_owner).as_deref() {
            Some("Neither") => DrawnOwner::Neither,
            Some("Both") => DrawnOwner::Both,
            _ => return Err(Malformed::Shape("drawn_owner is \"Neither\" or \"Both\"")),
        },
        objective: if name(&objective).as_deref() == Some("Line") {
            Objective::Line
        } else {
            const EXPECTED: &str = "an objective is \"Line\" or { FirstTo: N }";
            let n = tagged_data(&objective, "FirstTo").ok_or(Malformed::Shape(EXPECTED))?;
            Objective::FirstTo(byte(&n, EXPECTED)?)
        },
        fallback: match name(&fallback).as_deref() {
            Some("Draw") => Fallback::Draw,
            Some("MostWon") => Fallback::MostWon,
            _ => return Err(Malformed::Shape("fallback is \"Draw\" or \"MostWon\"")),
        },
        opening: if name(&opening).as_deref() == Some("Free") {
            Opening::Free
        } else {
            const EXPECTED: &str = "an opening is \"Free\" or { Random: k }";
            let k = tagged_data(&opening, "Random").ok_or(Malformed::Shape(EXPECTED))?;
            Opening::Random(byte(&k, EXPECTED)?)
        },
    })
}

pub fn parse_record(value: &JsValue) -> Result<Record, Malformed> {
    let [rules_version, ruleset, seed, moves] = fields(
        value,
        ["rules_version", "ruleset", "seed", "moves"],
        "a Record is { rules_version, ruleset, seed, moves }",
    )?;
    // No game has more moves than its 729 Cells.
    let moves = elements(&moves, 729, "a Record's moves are an array of Cells")?;
    Ok(Record {
        rules_version: whole(&rules_version)
            .ok_or(Malformed::Shape("rules_version is a whole number"))?,
        ruleset: parse_ruleset(&ruleset)?,
        seed: parse_seed(&seed)?,
        moves: moves.iter().map(parse_cell).collect::<Result<_, _>>()?,
    })
}

/// The root visits of every Worker's search together, as `Visit[]`: at most 4 searches of at
/// most 729 moves each.
pub fn parse_visits(value: &JsValue) -> Result<Vec<Visit>, Malformed> {
    const EXPECTED: &str = "the visits are an array of { cell, visits }";
    elements(value, 4 * 729, EXPECTED)?
        .iter()
        .map(|visit| {
            let [cell, visits] = fields(visit, ["cell", "visits"], EXPECTED)?;
            Ok(Visit {
                cell: parse_cell(&cell)?,
                visits: whole(&visits).ok_or(Malformed::Shape(EXPECTED))?,
            })
        })
        .collect()
}
