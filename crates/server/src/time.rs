//! The server's readings of time on the wire: Unix ms for a clock's `at` and a `Pong`, and whole
//! ms for a duration.

use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Now, in Unix ms.
pub fn unix_ms() -> u64 {
    let since = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    u64::try_from(since.as_millis()).unwrap_or(u64::MAX)
}

/// `duration` in whole ms, as the wire and the table count them.
pub fn ms(duration: Duration) -> u32 {
    u32::try_from(duration.as_millis()).unwrap_or(u32::MAX)
}
