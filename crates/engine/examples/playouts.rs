//! Random playouts per second at both depths, through the public API: the speed the computer's
//! search and the Level calibration rest on. A playout copies a fresh game's State and plays
//! uniformly random legal moves to the end, as the search's playouts do.
//!
//! `cargo run --release --example playouts [seconds per depth]`

use std::time::{Duration, Instant};

use kokonoe_engine::{Game, Preset, Seed, Status};

/// Playouts under `preset` for `time`, and how many that was per second.
fn playouts(preset: Preset, time: Duration) -> (u64, f64) {
    let start = Instant::now();
    let mut done = 0u64;
    let mut rng = 0x9E37_79B9_7F4A_7C15_u64;
    while start.elapsed() < time {
        let mut state = *Game::new(preset.ruleset(), Seed(done)).state();
        while let Status::ToMove { moves, .. } = state.status() {
            rng ^= rng << 13;
            rng ^= rng >> 7;
            rng ^= rng << 17;
            let cell = moves.nth((rng % moves.len() as u64) as usize);
            state.play(cell).expect("a legal move");
        }
        done += 1;
    }
    (done, done as f64 / start.elapsed().as_secs_f64())
}

fn main() {
    let seconds: f64 = std::env::args()
        .nth(1)
        .and_then(|arg| arg.parse().ok())
        .unwrap_or(3.0);
    for preset in [Preset::Standard, Preset::Deep] {
        let (done, rate) = playouts(preset, Duration::from_secs_f64(seconds));
        println!(
            "{:<9} depth {}  {rate:>9.0} playouts/s  ({done} in {seconds} s)",
            preset.name(),
            u8::from(preset.ruleset().depth)
        );
    }
}
