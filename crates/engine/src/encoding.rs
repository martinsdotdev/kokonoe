//! Text encodings (spec §4.1 to §4.3): Notation for Cells, the Ruleset code, and a Seed's hex
//! form. Each writes one spelling and its parser accepts only that spelling, apart from the `.`
//! Notation also takes, so every value has one encoding.

#[cfg(feature = "serde")]
use alloc::string::{String, ToString};
use core::fmt;
use core::str::FromStr;

use crate::address::Cell;
use crate::error::{CodeError, ParseError};
#[cfg(feature = "serde")]
use crate::play::Record;
use crate::play::Seed;
use crate::ruleset::{
    Continuation, Depth, DrawnOwner, Fallback, Goal, Objective, Opening, Preset, RandomPlies,
    Ruleset,
};

/// Notation: the Cell's path, each Space written 1 to 9 like a phone keypad, joined by `·`.
/// `5·5` at depth 2, `5·1·8` at depth 3.
impl fmt::Display for Cell {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (i, space) in self.spaces().iter().enumerate() {
            if i > 0 {
                f.write_str("·")?;
            }
            write!(f, "{}", space.index().wrapping_add(1))?;
        }
        Ok(())
    }
}

/// Reads Notation, with `.` accepted as well as `·`. The digit count is the depth.
impl FromStr for Cell {
    type Err = ParseError;

    fn from_str(text: &str) -> Result<Cell, ParseError> {
        let mut digits = [0; 3];
        let mut len = 0;
        for part in text.split(['·', '.']) {
            let [digit @ b'1'..=b'9'] = part.as_bytes() else {
                return Err(ParseError::Notation);
            };
            *digits.get_mut(len).ok_or(ParseError::Notation)? = digit.wrapping_sub(b'1');
            len = len.wrapping_add(1);
        }
        Cell::from_digits(digits.get(..len).unwrap_or(&[]))
    }
}

/// A move as move lists and text exports write it: in Notation, and in parentheses when the
/// Random opening dealt it, `(5·5)`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NotatedMove {
    pub cell: Cell,
    pub dealt: bool,
}

impl fmt::Display for NotatedMove {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.dealt {
            write!(f, "({})", self.cell)
        } else {
            write!(f, "{}", self.cell)
        }
    }
}

impl FromStr for NotatedMove {
    type Err = ParseError;

    fn from_str(text: &str) -> Result<NotatedMove, ParseError> {
        match text
            .strip_prefix('(')
            .and_then(|rest| rest.strip_suffix(')'))
        {
            Some(inner) => Ok(NotatedMove {
                cell: inner.parse()?,
                dealt: true,
            }),
            None => Ok(NotatedMove {
                cell: text.parse()?,
                dealt: false,
            }),
        }
    }
}

/// The settings a Ruleset code names. Declared in the code's order, so `Ord` is the order the
/// parser holds a code to.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Setting {
    Deep,
    Continuation,
    Both,
    FirstTo,
    MostWon,
    Random,
}

/// The Ruleset code: `std` for Standard, otherwise the settings that differ from it joined by
/// `+`, in the order `d3`, `cont`, `both`, `first<N>`, `most`, `rand<K>`.
impl fmt::Display for Ruleset {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut any = false;
        if self.depth == Depth::Three {
            setting(f, &mut any, format_args!("d3"))?;
        }
        if self.continuation == Continuation::On {
            setting(f, &mut any, format_args!("cont"))?;
        }
        if self.drawn_owner == DrawnOwner::Both {
            setting(f, &mut any, format_args!("both"))?;
        }
        if let Objective::FirstTo(goal) = self.objective {
            setting(f, &mut any, format_args!("first{}", goal.get()))?;
        }
        if self.fallback == Fallback::MostWon {
            setting(f, &mut any, format_args!("most"))?;
        }
        if let Opening::Random(plies) = self.opening {
            setting(f, &mut any, format_args!("rand{}", plies.get()))?;
        }
        if !any {
            f.write_str("std")?;
        }
        Ok(())
    }
}

/// Writes one setting of a code, after a `+` unless it's the first.
fn setting(f: &mut fmt::Formatter<'_>, any: &mut bool, text: fmt::Arguments<'_>) -> fmt::Result {
    if core::mem::replace(any, true) {
        f.write_str("+")?;
    }
    f.write_fmt(text)
}

/// Reads a Ruleset code, accepting only the spelling `Display` writes.
impl FromStr for Ruleset {
    type Err = ParseError;

    fn from_str(code: &str) -> Result<Ruleset, ParseError> {
        let mut rules = Preset::Standard.ruleset();
        if code == "std" {
            return Ok(rules);
        }
        let mut last = None;
        for token in code.split('+') {
            let setting = apply(&mut rules, token)?;
            match last {
                Some(previous) if previous == setting => return Err(CodeError::Duplicate.into()),
                Some(previous) if previous > setting => return Err(CodeError::OutOfOrder.into()),
                _ => last = Some(setting),
            }
        }
        Ok(rules)
    }
}

/// Sets what `token` names on `rules`, and says which setting it was, so the parser can hold
/// the code to one order with no repeats.
fn apply(rules: &mut Ruleset, token: &str) -> Result<Setting, ParseError> {
    let setting = match token {
        "" => return Err(CodeError::Empty.into()),
        "std" => return Err(CodeError::StdCombined.into()),
        "d3" => {
            rules.depth = Depth::Three;
            Setting::Deep
        }
        "cont" => {
            rules.continuation = Continuation::On;
            Setting::Continuation
        }
        "both" => {
            rules.drawn_owner = DrawnOwner::Both;
            Setting::Both
        }
        "most" => {
            rules.fallback = Fallback::MostWon;
            Setting::MostWon
        }
        _ if leading_zero(token) => return Err(CodeError::LeadingZero.into()),
        _ => {
            if let Some(n) = token.strip_prefix("first") {
                let goal = number(n).and_then(|n| Goal::try_from(n).ok());
                rules.objective = Objective::FirstTo(goal.ok_or(CodeError::FirstTo)?);
                Setting::FirstTo
            } else if let Some(k) = token.strip_prefix("rand") {
                let plies = number(k).and_then(|k| RandomPlies::try_from(k).ok());
                rules.opening = Opening::Random(plies.ok_or(CodeError::Random)?);
                Setting::Random
            } else {
                return Err(CodeError::Unknown.into());
            }
        }
    };
    Ok(setting)
}

/// Whether a numbered setting writes its number with a leading zero, as in `first05`.
fn leading_zero(token: &str) -> bool {
    ["first", "rand"].iter().any(|name| {
        token
            .strip_prefix(name)
            .is_some_and(|n| n.len() > 1 && n.starts_with('0'))
    })
}

/// A setting's number, if it's written as plain decimal digits. A leading zero is a second
/// spelling of the same number, so it's an error of its own; the range is the setting's to check.
fn number(digits: &str) -> Option<u8> {
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    digits.parse().ok()
}

/// A Seed as 16 lowercase hex digits, the u64 big-endian: a JSON number can't hold a u64 exactly
/// in JavaScript.
impl fmt::Display for Seed {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:016x}", self.0)
    }
}

/// Reads exactly the 16 lowercase hex digits `Display` writes.
impl FromStr for Seed {
    type Err = ParseError;

    fn from_str(hex: &str) -> Result<Seed, ParseError> {
        // from_str_radix alone would also take a sign and uppercase.
        if hex.len() != 16 || !hex.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f')) {
            return Err(ParseError::Seed);
        }
        u64::from_str_radix(hex, 16)
            .map(Seed)
            .map_err(|_| ParseError::Seed)
    }
}

/// For serde, which reads and writes a Seed through its hex form.
#[cfg(feature = "serde")]
impl From<Seed> for String {
    fn from(seed: Seed) -> String {
        seed.to_string()
    }
}

#[cfg(feature = "serde")]
impl TryFrom<String> for Seed {
    type Error = ParseError;

    fn try_from(hex: String) -> Result<Seed, ParseError> {
        hex.parse()
    }
}

/// Spec §4.3's encoded Record: the Record, plus the name a Custom variant was given. The engine
/// never needs the name, so it stays out of [`Record`].
#[cfg(feature = "serde")]
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct RecordFile {
    #[serde(flatten)]
    pub record: Record,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

/// A Record as its JSON carries it, with the moves as Cell indices: a Cell's depth comes from
/// the Ruleset, so they're read against it.
#[cfg(feature = "serde")]
#[derive(serde::Serialize, serde::Deserialize)]
pub(crate) struct RawRecord {
    rules_version: u32,
    ruleset: Ruleset,
    seed: Seed,
    moves: alloc::vec::Vec<u16>,
}

#[cfg(feature = "serde")]
impl From<Record> for RawRecord {
    fn from(record: Record) -> RawRecord {
        RawRecord {
            rules_version: record.rules_version,
            ruleset: record.ruleset,
            seed: record.seed,
            moves: record.moves.into_iter().map(Cell::index).collect(),
        }
    }
}

#[cfg(feature = "serde")]
impl TryFrom<RawRecord> for Record {
    type Error = ParseError;

    fn try_from(raw: RawRecord) -> Result<Record, ParseError> {
        let depth = raw.ruleset.depth;
        Ok(Record {
            rules_version: raw.rules_version,
            ruleset: raw.ruleset,
            seed: raw.seed,
            moves: raw
                .moves
                .into_iter()
                .map(|index| Cell::from_index(depth, index))
                .collect::<Result<_, _>>()?,
        })
    }
}
