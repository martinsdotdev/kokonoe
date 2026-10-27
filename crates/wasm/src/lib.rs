//! The engine's JavaScript bindings, the only crate that knows about the browser.
//!
//! One module serves the page and the AI Worker alike. [`Game`] wraps the engine's Game in WASM
//! memory, and [`think`] is the computer's search. The values they take and return are plain
//! JavaScript values: [`types`] declares them for the `.d.ts` as Rust mirrors, [`convert`] maps
//! the mirrors to and from the engine, and [`js`] builds and parses them. They cross as `Ts<T>`, a JS handle carrying its declared type,
//! and are parsed inside each method, so a value of the wrong shape is an ordinary error rather
//! than a throw from the ABI boundary.
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

mod convert;
mod js;
mod types;

use kokonoe_engine as engine;
use tsify::Ts;
use wasm_bindgen::prelude::*;

use js::ToJs;
use types::{
    Board, BoardStatus, Cell, PlayResult, Player, Ply, Preset, Record, ReplayError, Ruleset, Seed,
    Status,
};

/// A live game: its Record, the State it leads to, and the history between them.
#[wasm_bindgen]
pub struct Game(engine::Game);

#[wasm_bindgen]
impl Game {
    /// A new game, with a Random opening already dealt from `seed`.
    #[wasm_bindgen(constructor)]
    pub fn new(ruleset: Ts<Ruleset>, seed: Ts<Seed>) -> Result<Game, JsError> {
        let ruleset = engine::Ruleset::try_from(js::parse_ruleset(&ruleset.js_value())?)?;
        let seed = engine::Seed::try_from(&js::parse_seed(&seed.js_value())?)?;
        Ok(Game(engine::Game::new(ruleset, seed)))
    }

    /// The game a Record leads to, or the first of its moves the rules reject.
    #[wasm_bindgen(unchecked_return_type = "Game | ReplayError")]
    pub fn open(record: Ts<Record>) -> Result<JsValue, JsError> {
        let record = engine::Record::try_from(js::parse_record(&record.js_value())?)?;
        Ok(match engine::Game::open(&record) {
            Ok(game) => Game(game).into(),
            Err(error) => ReplayError::from(error).to_js(),
        })
    }

    /// Plays `cell` as the move at ply `at`. A move already made at `at`, a dealt one included,
    /// is `"Already"`, so a resend is harmless. Any other ply but the next is `OutOfOrder`.
    pub fn play(&mut self, at: Ts<Ply>, cell: Ts<Cell>) -> Result<Ts<PlayResult>, JsError> {
        let at = js::parse_ply(&at.js_value())?;
        let cell = js::parse_cell(&cell.js_value())?.at_depth(self.depth())?;
        let result = match at.in_range() {
            Some(ply) => PlayResult::from(self.0.play(ply, cell)),
            // Past any ply a game reaches, so never the next one.
            None => PlayResult::OutOfOrder {
                want: self.0.state().ply().into(),
                got: at,
            },
        };
        Ok(typed(result.to_js()))
    }

    /// Takes back the last move, never the dealt opening, and returns it.
    pub fn undo(&mut self) -> Option<Ts<Cell>> {
        self.0.undo().map(|cell| typed(Cell::from(cell).to_js()))
    }

    pub fn status(&self) -> Ts<Status> {
        typed(Status::from(self.0.state().status()).to_js())
    }

    pub fn ply(&self) -> Ts<Ply> {
        typed(Ply::from(self.0.state().ply()).to_js())
    }

    pub fn ruleset(&self) -> Ts<Ruleset> {
        typed(Ruleset::from(self.0.state().ruleset()).to_js())
    }

    pub fn mark(&self, cell: Ts<Cell>) -> Result<Option<Ts<Player>>, JsError> {
        let cell = js::parse_cell(&cell.js_value())?.at_depth(self.depth())?;
        let mark = self.0.state().mark(cell);
        Ok(mark.map(|player| typed(Player::from(player).to_js())))
    }

    #[wasm_bindgen(js_name = boardStatus)]
    pub fn board_status(&self, board: Ts<Board>) -> Result<Ts<BoardStatus>, JsError> {
        let board = js::parse_board(&board.js_value())?.at_depth(self.depth())?;
        Ok(typed(
            BoardStatus::from(self.0.state().board_status(board)).to_js(),
        ))
    }

    /// How many children of the Global board count as won for `player`: the ones it won, plus
    /// the drawn ones when drawn Boards count for both. FirstTo and MostWon compare this.
    #[wasm_bindgen(js_name = wonCount)]
    pub fn won_count(&self, player: Ts<Player>) -> Result<u8, JsError> {
        let player = js::parse_player(&player.js_value())?;
        Ok(self.0.state().won_count(player.into()))
    }

    pub fn record(&self) -> Ts<Record> {
        typed(Record::from(self.0.record()).to_js())
    }

    /// The moves players made, after the dealt opening.
    pub fn moves(&self) -> Vec<Ts<Cell>> {
        cells(self.0.moves())
    }

    /// The dealt opening, empty under a Free opening.
    pub fn opening(&self) -> Vec<Ts<Cell>> {
        cells(self.0.opening())
    }
}

impl Game {
    fn depth(&self) -> engine::Depth {
        self.0.state().ruleset().depth
    }
}

fn cells(cells: &[engine::Cell]) -> Vec<Ts<Cell>> {
    cells
        .iter()
        .map(|&cell| typed(Cell::from(cell).to_js()))
        .collect()
}

/// The computer's move in the game `record` leads to, at `level` 1 to 8, searched from `seed`
/// (spec §6). For the AI Worker: one call per move, so the search never crosses into JavaScript
/// except to read `Date.now` against the Level's time cap. A Level out of range, a Record the
/// rules reject and a finished game all throw, since the game screen never asks for them.
#[wasm_bindgen]
pub fn think(record: Ts<Record>, level: u8, seed: Ts<Seed>) -> Result<Ts<Cell>, JsError> {
    let record = engine::Record::try_from(js::parse_record(&record.js_value())?)?;
    let level = engine::Level::try_from(level)?;
    let seed = engine::Seed::try_from(&js::parse_seed(&seed.js_value())?)?;
    let game = engine::Game::open(&record)?;
    let start = js_sys::Date::now();
    let elapsed_ms = || (js_sys::Date::now() - start) as u32;
    let cell = engine::think(game.state(), level, seed.0, elapsed_ms)
        .ok_or_else(|| JsError::new("the game is over"))?;
    Ok(typed(Cell::from(cell).to_js()))
}

/// The five Presets, Standard first.
#[wasm_bindgen]
pub fn presets() -> Vec<Ts<Preset>> {
    engine::Preset::ALL
        .into_iter()
        .map(|preset| typed(Preset::from(preset).to_js()))
        .collect()
}

/// A value [`js`] built in the shape `T` declares.
fn typed<T: tsify::Tsify>(value: JsValue) -> Ts<T> {
    Ts::new_unchecked(value)
}
