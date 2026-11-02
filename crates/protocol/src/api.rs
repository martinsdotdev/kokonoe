//! The JSON of the Room routes: `POST /api/rooms` makes a Room, and `GET /api/rooms/{room}`
//! describes one to whoever holds its link.

use alloc::string::String;

use kokonoe_engine::Player;

use crate::{Players, TimeControl};

/// What the host chose: a Preset, by its Ruleset code, a seat, whether takebacks are allowed, and
/// the time control, untimed unless one is sent.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NewRoom {
    pub rules: String,
    pub seat: SeatChoice,
    #[cfg_attr(feature = "serde", serde(default))]
    pub takebacks: bool,
    pub clock: Option<TimeControl>,
}

/// The seat the host takes. `Random` is drawn when the Room is made.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SeatChoice {
    X,
    O,
    Random,
}

/// The Room that was made: its link is `/r/<room>`.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CreatedRoom {
    pub room: String,
}

/// What the joiner sees before accepting (spec §10.1). Every Room is Casual for now, so that
/// isn't said.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RoomCard {
    /// The host's Handle.
    pub host: String,
    /// The Variant's name.
    pub variant: String,
    /// Its Ruleset code.
    pub rules: String,
    pub players: Players,
    /// The seat the asker already holds, if they hold one.
    pub you: Option<Player>,
    /// Whether the Room allows takebacks.
    pub takebacks: bool,
    /// The time control, or None for an untimed Room.
    pub clock: Option<TimeControl>,
}

#[cfg(test)]
mod tests {
    use alloc::string::ToString;

    use super::*;

    #[test]
    fn a_room_is_asked_for_and_described() {
        let asked: NewRoom = serde_json::from_str(r#"{"rules":"std","seat":"Random"}"#).unwrap();
        assert_eq!(
            asked,
            NewRoom {
                rules: "std".to_string(),
                seat: SeatChoice::Random,
                takebacks: false,
                clock: None
            },
            "takebacks are off and the Room untimed unless asked for"
        );
        let timed: NewRoom =
            serde_json::from_str(r#"{"rules":"std","seat":"X","clock":"5+3"}"#).unwrap();
        assert_eq!(timed.clock, TimeControl::parse("5+3"));
        assert!(
            serde_json::from_str::<NewRoom>(r#"{"rules":"std","seat":"X","clock":"4+4"}"#).is_err(),
            "only the four controls"
        );
        let card = RoomCard {
            host: "guest-4821".to_string(),
            variant: "Standard".to_string(),
            rules: "std".to_string(),
            players: Players {
                x: Some("guest-4821".to_string()),
                o: None,
            },
            you: None,
            takebacks: true,
            clock: TimeControl::parse("10+5"),
        };
        assert_eq!(
            serde_json::to_string(&card).unwrap(),
            r#"{"host":"guest-4821","variant":"Standard","rules":"std","players":{"X":"guest-4821","O":null},"you":null,"takebacks":true,"clock":"10+5"}"#
        );
        let untimed = RoomCard {
            clock: None,
            ..card
        };
        assert!(
            serde_json::to_string(&untimed)
                .unwrap()
                .ends_with(r#""takebacks":true,"clock":null}"#)
        );
    }
}
