//! The engine's JavaScript bindings, the only crate that knows about the browser.
//!
//! One module serves the page and the AI Workers alike. [`Game`] wraps the engine's Game in WASM
//! memory. The computer's move is [`search`]ed in each Worker over its [`shares`] of the Level's
//! budget, and the page [`choose`]s from their visits. The values they take and return are plain
//! JavaScript values: [`types`] declares them for the `.d.ts` as Rust mirrors, [`convert`] maps
//! the mirrors to and from the engine, and [`js`] builds and parses them. They cross as `Ts<T>`,
//! a JS handle carrying its declared type, and are parsed inside each method, so a value of the
//! wrong shape is an ordinary error rather than a throw from the ABI boundary.
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
    Board, BoardStatus, Cell, PlayResult, Player, Ply, Preset, Record, ReplayError, Ruleset,
    Searched, Seed, Status,
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

    /// The game a Record leads to, or the first of its moves the rules reject. A Record from
    /// another rules version throws: this build can't say how it would replay.
    #[wasm_bindgen(unchecked_return_type = "Game | ReplayError")]
    pub fn open(record: Ts<Record>) -> Result<JsValue, JsError> {
        let record = engine::Record::try_from(js::parse_record(&record.js_value())?)?;
        Ok(match engine::Game::open(&record) {
            Ok(game) => Game(game).into(),
            Err(engine::OpenError::Illegal(error)) => ReplayError::from(error).to_js(),
            Err(error @ engine::OpenError::RulesVersion(_)) => return Err(error.into()),
        })
    }

    /// Plays `cell` as the move at ply `at`. A move already made at `at`, a dealt one included,
    /// is `"Already"`, so a resend is harmless. Any other ply but the next is `OutOfOrder`.
    pub fn play(&mut self, at: Ts<Ply>, cell: Ts<Cell>) -> Result<Ts<PlayResult>, JsError> {
        let at = js::parse_ply(&at.js_value())?;
        let cell = js::parse_cell(&cell.js_value())?.at_depth(self.0.state().ruleset().depth)?;
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
        read::status(self.0.state())
    }

    pub fn ply(&self) -> Ts<Ply> {
        read::ply(self.0.state())
    }

    pub fn ruleset(&self) -> Ts<Ruleset> {
        read::ruleset(self.0.state())
    }

    pub fn mark(&self, cell: Ts<Cell>) -> Result<Option<Ts<Player>>, JsError> {
        read::mark(self.0.state(), cell)
    }

    #[wasm_bindgen(js_name = boardStatus)]
    pub fn board_status(&self, board: Ts<Board>) -> Result<Ts<BoardStatus>, JsError> {
        read::board_status(self.0.state(), board)
    }

    /// How many children of the Global board count as won for `player`: the ones it won, plus
    /// the drawn ones when drawn Boards count for both. FirstTo and MostWon compare this.
    #[wasm_bindgen(js_name = wonCount)]
    pub fn won_count(&self, player: Ts<Player>) -> Result<u8, JsError> {
        read::won_count(self.0.state(), player)
    }

    /// The State after `at` plies, the dealt opening included, where 0 is the empty board: a
    /// replay's position at any ply. `undefined` past the last ply.
    #[wasm_bindgen(js_name = stateAt)]
    pub fn state_at(&self, at: Ts<Ply>) -> Result<Option<State>, JsError> {
        let at = js::parse_ply(&at.js_value())?;
        Ok(at
            .in_range()
            .and_then(|ply| self.0.state_at(ply))
            .map(State))
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

/// The game at one ply, to read and not to play on: what [`Game::state_at`] returns.
#[wasm_bindgen]
pub struct State(engine::State);

#[wasm_bindgen]
impl State {
    pub fn status(&self) -> Ts<Status> {
        read::status(&self.0)
    }

    pub fn ply(&self) -> Ts<Ply> {
        read::ply(&self.0)
    }

    pub fn ruleset(&self) -> Ts<Ruleset> {
        read::ruleset(&self.0)
    }

    pub fn mark(&self, cell: Ts<Cell>) -> Result<Option<Ts<Player>>, JsError> {
        read::mark(&self.0, cell)
    }

    #[wasm_bindgen(js_name = boardStatus)]
    pub fn board_status(&self, board: Ts<Board>) -> Result<Ts<BoardStatus>, JsError> {
        read::board_status(&self.0, board)
    }

    /// How many children of the Global board count as won for `player`, as a Game counts them.
    #[wasm_bindgen(js_name = wonCount)]
    pub fn won_count(&self, player: Ts<Player>) -> Result<u8, JsError> {
        read::won_count(&self.0, player)
    }
}

/// What a Game and a State read alike, from one engine State.
mod read {
    use super::*;

    pub fn status(state: &engine::State) -> Ts<Status> {
        typed(Status::from(state.status()).to_js())
    }

    pub fn ply(state: &engine::State) -> Ts<Ply> {
        typed(Ply::from(state.ply()).to_js())
    }

    pub fn ruleset(state: &engine::State) -> Ts<Ruleset> {
        typed(Ruleset::from(state.ruleset()).to_js())
    }

    pub fn mark(state: &engine::State, cell: Ts<Cell>) -> Result<Option<Ts<Player>>, JsError> {
        let cell = js::parse_cell(&cell.js_value())?.at_depth(state.ruleset().depth)?;
        Ok(state
            .mark(cell)
            .map(|player| typed(Player::from(player).to_js())))
    }

    pub fn board_status(
        state: &engine::State,
        board: Ts<Board>,
    ) -> Result<Ts<BoardStatus>, JsError> {
        let board = js::parse_board(&board.js_value())?.at_depth(state.ruleset().depth)?;
        Ok(typed(BoardStatus::from(state.board_status(board)).to_js()))
    }

    pub fn won_count(state: &engine::State, player: Ts<Player>) -> Result<u8, JsError> {
        let player = js::parse_player(&player.js_value())?;
        Ok(state.won_count(player.into()))
    }
}

fn cells(cells: &[engine::Cell]) -> Vec<Ts<Cell>> {
    cells
        .iter()
        .map(|&cell| typed(Cell::from(cell).to_js()))
        .collect()
}

/// One Worker's search for the computer's move in the game `record` leads to, at `level` 1 to 8
/// (spec §6): `iterations` of them, its share of the Level's budget, from `seed`. The page asked
/// at `started_at`, its `Date.now()`, and the Level's time cap counts from then, so every Worker stops
/// by the same moment. A Level out of range, a Record the rules reject and a finished game all
/// throw, since the game screen never asks for them.
#[wasm_bindgen]
pub fn search(
    record: Ts<Record>,
    level: u8,
    iterations: u32,
    seed: Ts<Seed>,
    #[wasm_bindgen(js_name = startedAt)] started_at: f64,
) -> Result<Ts<Searched>, JsError> {
    let record = engine::Record::try_from(js::parse_record(&record.js_value())?)?;
    let level = engine::Level::try_from(level)?;
    let seed = engine::Seed::try_from(&js::parse_seed(&seed.js_value())?)?;
    let game = engine::Game::open(&record)?;
    // Saturating: a clock read before `started_at` is no time at all.
    let elapsed_ms = || (js_sys::Date::now() - started_at) as u32;
    let searched = engine::search(game.state(), level, iterations, seed.0, elapsed_ms)
        .ok_or_else(|| JsError::new("the game is over"))?;
    Ok(typed(Searched::from(searched).to_js()))
}

/// The iterations each Worker searches at `level` on a device with `cores` logical cores, one
/// share for each of the engine's `Level::workers`, summing to the Level's budget.
#[wasm_bindgen]
pub fn shares(level: u8, cores: u32) -> Result<Vec<u32>, JsError> {
    Ok(engine::shares(engine::Level::try_from(level)?, cores))
}

/// The move `level` plays from its Workers' visits, all of them together, in a game of `depth`:
/// summed by Cell, then the most visited or one sampled, from `seed`.
#[wasm_bindgen]
pub fn choose(
    level: u8,
    #[wasm_bindgen(unchecked_param_type = "2 | 3")] depth: u8,
    #[wasm_bindgen(unchecked_param_type = "Visit[]")] visits: JsValue,
    seed: Ts<Seed>,
) -> Result<Ts<Cell>, JsError> {
    let level = engine::Level::try_from(level)?;
    let depth = engine::Depth::try_from(depth)?;
    let seed = engine::Seed::try_from(&js::parse_seed(&seed.js_value())?)?;
    let visits = js::parse_visits(&visits)?
        .into_iter()
        .map(|visit| visit.at_depth(depth))
        .collect::<Result<Vec<_>, _>>()?;
    let cell = engine::choose(level, &visits, seed.0)
        .ok_or_else(|| JsError::new("no search listed a move"))?;
    Ok(typed(Cell::from(cell).to_js()))
}

/// The Ruleset code of spec §4.2: `std`, or the settings that differ from Standard.
#[wasm_bindgen(js_name = rulesetCode)]
pub fn ruleset_code(ruleset: Ts<Ruleset>) -> Result<String, JsError> {
    let ruleset = engine::Ruleset::try_from(js::parse_ruleset(&ruleset.js_value())?)?;
    Ok(ruleset.to_string())
}

/// The Ruleset a code names. A code with the wrong spelling throws the reason, since codes
/// arrive in links people type.
#[wasm_bindgen(js_name = parseRulesetCode)]
pub fn parse_ruleset_code(code: &str) -> Result<Ts<Ruleset>, JsError> {
    let ruleset: engine::Ruleset = code.parse()?;
    Ok(typed(Ruleset::from(ruleset).to_js()))
}

/// A Cell in Notation: `5·5` at depth 2, `5·1·8` at depth 3.
#[wasm_bindgen]
pub fn notation(
    cell: Ts<Cell>,
    #[wasm_bindgen(unchecked_param_type = "2 | 3")] depth: u8,
) -> Result<String, JsError> {
    let depth = engine::Depth::try_from(depth)?;
    let cell = js::parse_cell(&cell.js_value())?.at_depth(depth)?;
    Ok(cell.to_string())
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
