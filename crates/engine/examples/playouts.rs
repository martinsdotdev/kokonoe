//! Random playouts per second at both depths: the speed the computer's search and the Level
//! calibration rest on. Each playout starts from a fresh game and plays uniformly random legal
//! moves to the end, two ways: `State::playout`, which the search uses, and `play` with
//! `LegalMoves::nth`, which builds the full Status after every move as a screen needs.
//!
//! `cargo run --release --example playouts [seconds per row]`

use std::time::{Duration, Instant};

use kokonoe_engine::{Game, Preset, Seed, State, Status};

/// One way to play a game out, given its starting State and a generator's state.
type PlayOut<'a> = &'a dyn Fn(State, &mut u64);

/// xorshift, so the engine's own generators stay out of the measurement.
fn next(rng: &mut u64) -> u64 {
    *rng ^= *rng << 13;
    *rng ^= *rng >> 7;
    *rng ^= *rng << 17;
    *rng
}

/// Playouts under `preset` for `time`, each made by `play_out`, and how many that was per second.
fn rate(preset: Preset, time: Duration, play_out: impl Fn(State, &mut u64)) -> (u64, f64) {
    let start = Instant::now();
    let mut done = 0u64;
    let mut rng = 0x9E37_79B9_7F4A_7C15_u64;
    while start.elapsed() < time {
        play_out(*Game::new(preset.ruleset(), Seed(done)).state(), &mut rng);
        done += 1;
    }
    (done, done as f64 / start.elapsed().as_secs_f64())
}

fn main() {
    let seconds: f64 = std::env::args()
        .nth(1)
        .and_then(|arg| arg.parse().ok())
        .unwrap_or(3.0);
    let time = Duration::from_secs_f64(seconds);
    for preset in [Preset::Standard, Preset::Deep] {
        let depth = u8::from(preset.ruleset().depth);
        let rows: [(&str, PlayOut); 2] = [
            ("State::playout", &|mut state, rng| {
                state.playout(|n| (next(rng) % n as u64) as usize);
            }),
            ("play + nth", &|mut state, rng| {
                while let Status::ToMove { moves, .. } = state.status() {
                    let cell = moves.nth((next(rng) % moves.len() as u64) as usize);
                    state.play(cell).expect("a legal move");
                }
            }),
        ];
        for (name, play_out) in rows {
            let (done, per_second) = rate(preset, time, play_out);
            println!(
                "depth {depth}  {name:<15} {per_second:>9.0} playouts/s  ({done} in {seconds} s)"
            );
        }
    }
}
