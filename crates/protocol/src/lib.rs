//! What the server and its clients say to each other (spec §10.3), shared by the server and,
//! through `kokonoe-wasm`, the SPA: the codes the server closes a WebSocket with.

#![no_std]

/// Why the server closed a WebSocket, as its close code.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u16)]
pub enum CloseCode {
    /// The server is restarting for a deploy (spec §8): reconnect once it's back. The standard
    /// code for a service restart.
    Restarting = 1012,
    /// The client's build is older than the server will play with: activate the newer build at
    /// once and reload, which `updateNow` in `web/src/lib/update.ts` does (spec §5.1).
    UpdateRequired = 4001,
}

impl CloseCode {
    /// Every code, for the mirrors that must name each one.
    pub const ALL: [CloseCode; 2] = [CloseCode::Restarting, CloseCode::UpdateRequired];

    /// The number sent in the WebSocket's close frame.
    pub const fn code(self) -> u16 {
        self as u16
    }
}

impl TryFrom<u16> for CloseCode {
    /// A code this protocol doesn't name, as it was.
    type Error = u16;

    fn try_from(code: u16) -> Result<CloseCode, u16> {
        CloseCode::ALL
            .into_iter()
            .find(|close| close.code() == code)
            .ok_or(code)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_code_reads_back_as_itself() {
        for close in CloseCode::ALL {
            assert_eq!(CloseCode::try_from(close.code()), Ok(close));
        }
        assert_eq!(CloseCode::UpdateRequired.code(), 4001);
        assert_eq!(CloseCode::Restarting.code(), 1012);
        assert_eq!(CloseCode::try_from(1000), Err(1000));
    }
}
