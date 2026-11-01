//! The JSON of the Room routes: `POST /api/rooms` makes a Room, and `GET /api/rooms/{room}`
//! describes one to whoever holds its link.

use alloc::string::String;

use kokonoe_engine::Player;

use crate::Players;

/// What the host chose: a Preset, by its Ruleset code, and a seat.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NewRoom {
    pub rules: String,
    pub seat: SeatChoice,
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

/// What the joiner sees before accepting (spec §10.1). Every Room is Casual and untimed for now,
/// so that isn't said.
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
                seat: SeatChoice::Random
            }
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
        };
        assert_eq!(
            serde_json::to_string(&card).unwrap(),
            r#"{"host":"guest-4821","variant":"Standard","rules":"std","players":{"X":"guest-4821","O":null},"you":null}"#
        );
    }
}
