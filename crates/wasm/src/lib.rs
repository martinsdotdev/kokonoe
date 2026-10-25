//! The engine's JavaScript bindings, the only crate that knows about the browser.
//!
//! One module serves the page and the AI Worker alike. [`Game`] wraps the engine's Game in WASM
//! memory. The values it takes and returns are plain JavaScript values, typed in the `.d.ts` by
//! the declarations in [`types`] and converted by hand in [`js`]. They cross as `Ts<T>`, a JS
//! handle carrying its declared type, and are parsed inside each method, so a value of the wrong
//! shape is an ordinary error rather than a throw from the ABI boundary.
//!
//! Errors keep the engine's two layers. A move the rules refuse comes back as a value
//! (`PlayResult`, `ReplayError`), so the UI can name the Board it flashes. Data with the wrong
//! shape (a bad Seed, a Cell index past the last Cell, a Record from another rules version)
//! throws a JavaScript `Error`, since a typed UI never sends it.

#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects
)]

mod js;
mod types;

use kokonoe_engine as engine;
use tsify::Ts;
use wasm_bindgen::prelude::*;

use js::ToJs;
use types::{
    Board, BoardStatus, Cell, PlayResult, Player, Ply, Preset, Record, Ruleset, Seed, Status,
};

/// A live game: its Record, the State it leads to, and the history between them.
#[wasm_bindgen]
pub struct Game(engine::Game);

#[wasm_bindgen]
impl Game {
    /// A new game, with a Random opening already dealt from `seed`.
    #[wasm_bindgen(constructor)]
    pub fn new(ruleset: Ts<Ruleset>, seed: Ts<Seed>) -> Result<Game, JsError> {
        let ruleset = js::parse_ruleset(&ruleset.js_value())?;
        let seed = js::parse_seed(&seed.js_value())?;
        Ok(Game(engine::Game::new(ruleset, seed)))
    }

    /// The game a Record leads to, or the first of its moves the rules reject.
    #[wasm_bindgen(unchecked_return_type = "Game | ReplayError")]
    pub fn open(record: Ts<Record>) -> Result<JsValue, JsError> {
        let record = js::parse_record(&record.js_value())?;
        Ok(match engine::Game::open(&record) {
            Ok(game) => Game(game).into(),
            Err(error) => error.to_js(),
        })
    }

    /// Plays `cell` as the move at ply `at`. A resend of the last move is `"Already"`, so a
    /// retry is harmless.
    pub fn play(&mut self, at: Ts<Ply>, cell: Ts<Cell>) -> Result<Ts<PlayResult>, JsError> {
        let at = js::parse_ply(&at.js_value())?;
        let cell = js::parse_cell(&cell.js_value(), self.depth())?;
        Ok(typed(self.0.play(at, cell).to_js()))
    }

    /// Takes back the last move, never the dealt opening, and returns it.
    pub fn undo(&mut self) -> Option<Ts<Cell>> {
        self.0.undo().map(|cell| typed(cell.to_js()))
    }

    pub fn status(&self) -> Ts<Status> {
        typed(self.0.state().status().to_js())
    }

    pub fn ply(&self) -> Ts<Ply> {
        typed(self.0.state().ply().to_js())
    }

    pub fn ruleset(&self) -> Ts<Ruleset> {
        typed(self.0.state().ruleset().to_js())
    }

    pub fn mark(&self, cell: Ts<Cell>) -> Result<Option<Ts<Player>>, JsError> {
        let cell = js::parse_cell(&cell.js_value(), self.depth())?;
        Ok(self
            .0
            .state()
            .mark(cell)
            .map(|player| typed(player.to_js())))
    }

    #[wasm_bindgen(js_name = boardStatus)]
    pub fn board_status(&self, board: Ts<Board>) -> Result<Ts<BoardStatus>, JsError> {
        let board = js::parse_board(&board.js_value(), self.depth())?;
        Ok(typed(self.0.state().board_status(board).to_js()))
    }

    /// How many children of the Global board `player` has won.
    #[wasm_bindgen(js_name = wonCount)]
    pub fn won_count(&self, player: Ts<Player>) -> Result<u8, JsError> {
        let player = js::parse_player(&player.js_value())?;
        Ok(self.0.state().won_count(player))
    }

    pub fn record(&self) -> Ts<Record> {
        typed(self.0.record().to_js())
    }

    /// The moves players made, after the dealt opening.
    pub fn moves(&self) -> Vec<Ts<Cell>> {
        self.0
            .moves()
            .iter()
            .map(|cell| typed(cell.to_js()))
            .collect()
    }

    /// The dealt opening, empty under a Free opening.
    pub fn opening(&self) -> Vec<Ts<Cell>> {
        self.0
            .opening()
            .iter()
            .map(|cell| typed(cell.to_js()))
            .collect()
    }
}

impl Game {
    fn depth(&self) -> engine::Depth {
        self.0.state().ruleset().depth
    }
}

/// The five Presets, Standard first.
#[wasm_bindgen]
pub fn presets() -> Vec<Ts<Preset>> {
    engine::Preset::ALL
        .into_iter()
        .map(|preset| typed(preset.to_js()))
        .collect()
}

/// A value built by [`js`] in the shape `T` declares.
fn typed<T: tsify::Tsify>(value: JsValue) -> Ts<T> {
    Ts::new_unchecked(value)
}
