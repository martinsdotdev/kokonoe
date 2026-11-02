//! Timed Rooms (spec §11): a Room's time control, and its clocks as a page hears them.

use core::fmt;

use kokonoe_engine::Player;

/// A Fischer time control: a bank in minutes, and an increment in seconds after each move. Only
/// the four spec §10.1 offers exist; on the wire, and in the `games` table, each is spelled as a
/// game file's `TimeControl` tag, `5+3`.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TimeControl {
    #[cfg_attr(feature = "serde", serde(rename = "3+2"))]
    ThreePlusTwo,
    #[cfg_attr(feature = "serde", serde(rename = "5+3"))]
    FivePlusThree,
    #[cfg_attr(feature = "serde", serde(rename = "10+5"))]
    TenPlusFive,
    #[cfg_attr(feature = "serde", serde(rename = "15+5"))]
    FifteenPlusFive,
}

impl TimeControl {
    pub const ALL: [TimeControl; 4] = [
        TimeControl::ThreePlusTwo,
        TimeControl::FivePlusThree,
        TimeControl::TenPlusFive,
        TimeControl::FifteenPlusFive,
    ];

    /// Each player's bank, in minutes.
    pub const fn minutes(self) -> u8 {
        match self {
            TimeControl::ThreePlusTwo => 3,
            TimeControl::FivePlusThree => 5,
            TimeControl::TenPlusFive => 10,
            TimeControl::FifteenPlusFive => 15,
        }
    }

    /// What each move adds, in seconds.
    pub const fn increment(self) -> u8 {
        match self {
            TimeControl::ThreePlusTwo => 2,
            TimeControl::FivePlusThree => 3,
            TimeControl::TenPlusFive | TimeControl::FifteenPlusFive => 5,
        }
    }

    /// The control as spec §4.4 spells it, on the wire and in the `games` table.
    pub const fn as_str(self) -> &'static str {
        match self {
            TimeControl::ThreePlusTwo => "3+2",
            TimeControl::FivePlusThree => "5+3",
            TimeControl::TenPlusFive => "10+5",
            TimeControl::FifteenPlusFive => "15+5",
        }
    }

    /// The control `text` spells, if it spells one.
    pub fn parse(text: &str) -> Option<TimeControl> {
        TimeControl::ALL
            .into_iter()
            .find(|control| control.as_str() == text)
    }
}

impl fmt::Display for TimeControl {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A timed game's clocks as they stood at `at` (spec §10.3).
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Clocks {
    /// Each player's time left, in ms.
    pub x_ms: u32,
    pub o_ms: u32,
    /// Whose clock runs: the player to move, once they've made their first move.
    pub running: Option<Player>,
    /// The server's Unix time, in ms, when the clocks stood so. A page reads its offset from the
    /// server's time by `Ping`.
    pub at: u64,
    /// When, in the server's Unix ms, the game aborts unless the player to move makes their first
    /// move; None once both players have.
    pub aborts_at: Option<u64>,
}

/// Why a game ended without a result.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Abort {
    /// A player didn't make their first move in time.
    FirstMoveTimeout,
}

#[cfg(test)]
mod tests {
    use alloc::format;
    use alloc::string::ToString;

    use super::*;

    #[test]
    fn only_the_four_controls_are_spelled() {
        for (text, minutes, increment) in [
            ("3+2", 3, 2),
            ("5+3", 5, 3),
            ("10+5", 10, 5),
            ("15+5", 15, 5),
        ] {
            let control = TimeControl::parse(text).unwrap();
            assert_eq!(
                (control.minutes(), control.increment()),
                (minutes, increment)
            );
            assert_eq!(control.to_string(), text);
            assert_eq!(
                serde_json::to_string(&control).unwrap(),
                format!("\"{text}\"")
            );
            assert_eq!(
                serde_json::from_str::<TimeControl>(&format!("\"{text}\"")).unwrap(),
                control
            );
        }
        for text in ["4+4", "5+03", "05+3", "5", "+3", "5+3+1", "", "-"] {
            assert_eq!(TimeControl::parse(text), None, "{text}");
            assert!(serde_json::from_str::<TimeControl>(&format!("\"{text}\"")).is_err());
        }
    }
}
