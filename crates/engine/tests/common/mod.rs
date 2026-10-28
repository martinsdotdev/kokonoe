//! Helpers the engine's tests share: Cells written as digits, games built from them, and a small
//! generator for random games that stays independent of the engine's own.

// Each test crate compiles its own copy and uses only part of it.
#![allow(dead_code)]

use kokonoe_engine::{Cell, Game, Ruleset, Seed};

/// Space-separated Cells, each written as its digits, two or three: `"42"` is `4·2` and `"407"`
/// is `4·0·7`.
pub fn cells(text: &str) -> Vec<Cell> {
    text.split_whitespace()
        .map(|digits| {
            let digits: Vec<u8> = digits.bytes().map(|b| b - b'0').collect();
            Cell::from_digits(&digits).unwrap()
        })
        .collect()
}

/// Plays `cell` as the next move.
pub fn advance(game: &mut Game, cell: Cell) {
    let at = game.state().ply();
    game.play(at, cell).unwrap();
}

/// A game under `rules`, from Seed 0, with `moves` played in order.
pub fn game(rules: Ruleset, moves: &str) -> Game {
    let mut game = Game::new(rules, Seed(0));
    for cell in cells(moves) {
        advance(&mut game, cell);
    }
    game
}

/// xorshift64, for random games independent of the engine's generators. Any seed but 0.
pub struct Xorshift(pub u64);

impl Xorshift {
    pub fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    /// In `0..n`, for picking among `n` moves.
    pub fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
}
