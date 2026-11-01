//! Unguessable values: the bytes behind session tokens, Room ids and Seeds, and their hex.

use std::fmt::Write;

/// `N` bytes from the operating system's generator, which fails only on a broken system.
pub fn bytes<const N: usize>() -> [u8; N] {
    let mut bytes = [0; N];
    getrandom::fill(&mut bytes).expect("the operating system has no random numbers");
    bytes
}

/// `bytes` as lowercase hex, two digits a byte.
pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().fold(String::new(), |mut text, byte| {
        let _ = write!(text, "{byte:02x}");
        text
    })
}
