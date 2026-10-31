/* tslint:disable */
/* eslint-disable */
/**
 * A Board as its Space path from the Global board down, each Space 0 to 8: `[]` is the Global
 * board and `[4]` is the middle Local board at depth 2.
 */
export type Board = number[];

/**
 * A Cell as its base-9 index: 0 to 80 at depth 2 and 0 to 728 at depth 3, the first Space being
 * the most significant digit. The game supplies the depth.
 */
export type Cell = number;

/**
 * A Cell the rules don't allow now. Each variant names the Board the UI flashes.
 */
export type IllegalMove = "GameOver" | "WrongDepth" | "Occupied" | { BeneathDecided: { board: Board } } | { OutsideActiveBoards: { within: Board } };

/**
 * A Seed as 16 lowercase hex digits, since a JSON number can't hold a u64 exactly in
 * JavaScript.
 */
export type Seed = string;

/**
 * A game as data, in spec §4.3's JSON: what storage keeps, an export carries and a Worker
 * receives. `moves` excludes the dealt opening, which `seed` determines.
 */
export interface Record {
    rules_version: number;
    ruleset: Ruleset;
    seed: Seed;
    moves: Cell[];
}

/**
 * How many marks are on the board, the dealt opening included.
 */
export type Ply = number;

/**
 * One root move's visits from a search: how many of its iterations went through `cell`.
 */
export interface Visit {
    cell: Cell;
    count: number;
}

/**
 * The constraint the last move places on the side to move. Under `Free`, the Sent target can
 * take no move, so the player may play anywhere legal inside `within`.
 */
export type Sent = "FirstMove" | { To: Board } | { Free: { target: Board; within: Board } };

/**
 * The first move of a Record that the rules reject, as an index into the Record's moves.
 */
export interface ReplayError {
    index: number;
    error: IllegalMove;
}

/**
 * The game's progress. `moves` holds exactly the legal Cells, in ascending index order, and is
 * never empty.
 */
export type Status = { ToMove: { player: Player; sent: Sent; moves: Cell[] } } | { Over: Outcome };

/**
 * The six rule settings, in spec §4.2's JSON. Standard is `{"depth":2,"continuation":"Off",
 * "drawn_owner":"Neither","objective":"Line","fallback":"Draw","opening":"Free"}`.
 */
export interface Ruleset {
    depth: 2 | 3;
    continuation: Continuation;
    drawn_owner: DrawnOwner;
    objective: Objective;
    fallback: Fallback;
    opening: Opening;
}

/**
 * What `play` did: the move went in (`Now`), was already made at that ply (`Already`), or was
 * refused, as an illegal move or a ply out of order.
 */
export type PlayResult = { Now: Status } | "Already" | { Illegal: IllegalMove } | { OutOfOrder: { want: Ply; got: Ply } };

/**
 * What one search from a root found: a move the tactics force, or the visits of every root move
 * it could play, in ascending Cell index.
 */
export type Searched = { Forced: Cell } | { Visits: Visit[] };

/**
 * `FirstTo` holds N, 1 to 9.
 */
export type Objective = "Line" | { FirstTo: number };

/**
 * `Random` holds k, the dealt plies, 1 to 8.
 */
export type Opening = "Free" | { Random: number };

export interface Preset {
    name: string;
    ruleset: Ruleset;
}

export type BoardStatus = "Open" | { Won: Player } | "Drawn";

export type Continuation = "Off" | "On";

export type DrawnOwner = "Neither" | "Both";

export type Fallback = "Draw" | "MostWon";

export type Outcome = { Objective: Player } | { MostWon: Player } | "Draw";

export type Player = "X" | "O";


/**
 * Why the server closed a WebSocket (spec §10.3), as `kokonoe-protocol` names it: compare a
 * `CloseEvent`'s `code` with these. 4001 asks for the newer build, which `updateNow` activates.
 * Unlike the tsify types, it's a runtime enum, so the page names a code and its number stays in
 * Rust.
 */
export enum CloseCode {
    Restarting = 1012,
    UpdateRequired = 4001,
}

/**
 * A live game: its Record, the State it leads to, and the history between them.
 */
export class Game {
    free(): void;
    [Symbol.dispose](): void;
    boardStatus(board: Board): BoardStatus;
    mark(cell: Cell): Player | undefined;
    /**
     * The moves players made, after the dealt opening.
     */
    moves(): Cell[];
    /**
     * A new game, with a Random opening already dealt from `seed`.
     */
    constructor(ruleset: Ruleset, seed: Seed);
    /**
     * The game a Record leads to, or the first of its moves the rules reject. A Record from
     * another rules version throws: this build can't say how it would replay.
     */
    static open(record: Record): Game | ReplayError;
    /**
     * The dealt opening, empty under a Free opening.
     */
    opening(): Cell[];
    /**
     * Plays `cell` as the move at ply `at`. A move already made at `at`, a dealt one included,
     * is `"Already"`, so a resend is harmless. Any other ply but the next is `OutOfOrder`.
     */
    play(at: Ply, cell: Cell): PlayResult;
    ply(): Ply;
    record(): Record;
    ruleset(): Ruleset;
    /**
     * The State after `at` plies, the dealt opening included, where 0 is the empty board: a
     * replay's position at any ply. `undefined` past the last ply.
     */
    stateAt(at: Ply): State | undefined;
    status(): Status;
    /**
     * Takes back the last move, never the dealt opening, and returns it.
     */
    undo(): Cell | undefined;
    /**
     * How many children of the Global board count as won for `player`: the ones it won, plus
     * the drawn ones when drawn Boards count for both. FirstTo and MostWon compare this.
     */
    wonCount(player: Player): number;
}

/**
 * The game at one ply, to read and not to play on: what [`Game::state_at`] returns.
 */
export class State {
    private constructor();
    free(): void;
    [Symbol.dispose](): void;
    boardStatus(board: Board): BoardStatus;
    mark(cell: Cell): Player | undefined;
    ply(): Ply;
    ruleset(): Ruleset;
    status(): Status;
    /**
     * How many children of the Global board count as won for `player`, as a Game counts them.
     */
    wonCount(player: Player): number;
}

/**
 * The move `level` plays from its Workers' visits, all of them together, in a game of `depth`:
 * summed by Cell, then the most visited or one sampled, from `seed`.
 */
export function choose(level: number, depth: 2 | 3, visits: Visit[], seed: Seed): Cell;

/**
 * A Cell in Notation: `5·5` at depth 2, `5·1·8` at depth 3.
 */
export function notation(cell: Cell, depth: 2 | 3): string;

/**
 * The Ruleset a code names. A code with the wrong spelling throws the reason, since codes
 * arrive in links people type.
 */
export function parseRulesetCode(code: string): Ruleset;

/**
 * The five Presets, Standard first.
 */
export function presets(): Preset[];

/**
 * The Ruleset code of spec §4.2: `std`, or the settings that differ from Standard.
 */
export function rulesetCode(ruleset: Ruleset): string;

/**
 * One Worker's search for the computer's move in the game `record` leads to, at `level` 1 to 8
 * (spec §6): `iterations` of them, its share of the Level's budget, from `seed`. The page asked
 * at `started_at`, its `Date.now()`, and the Level's time cap counts from then, so every Worker stops
 * by the same moment. A Level out of range, a Record the rules reject and a finished game all
 * throw, since the game screen never asks for them.
 */
export function search(record: Record, level: number, iterations: number, seed: Seed, startedAt: number): Searched;

/**
 * The iterations each Worker searches at `level` on a device with `cores` logical cores, one
 * share for each of the engine's `Level::workers`, summing to the Level's budget.
 */
export function shares(level: number, cores: number): Uint32Array;

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
    readonly memory: WebAssembly.Memory;
    readonly __wbg_game_free: (a: number, b: number) => void;
    readonly __wbg_state_free: (a: number, b: number) => void;
    readonly choose: (a: number, b: number, c: any, d: any) => [number, number, number];
    readonly game_boardStatus: (a: number, b: any) => [number, number, number];
    readonly game_mark: (a: number, b: any) => [number, number, number];
    readonly game_moves: (a: number) => [number, number];
    readonly game_new: (a: any, b: any) => [number, number, number];
    readonly game_open: (a: any) => [number, number, number];
    readonly game_opening: (a: number) => [number, number];
    readonly game_play: (a: number, b: any, c: any) => [number, number, number];
    readonly game_ply: (a: number) => any;
    readonly game_record: (a: number) => any;
    readonly game_ruleset: (a: number) => any;
    readonly game_stateAt: (a: number, b: any) => [number, number, number];
    readonly game_status: (a: number) => any;
    readonly game_undo: (a: number) => any;
    readonly game_wonCount: (a: number, b: any) => [number, number, number];
    readonly notation: (a: any, b: number) => [number, number, number, number];
    readonly parseRulesetCode: (a: number, b: number) => [number, number, number];
    readonly presets: () => [number, number];
    readonly rulesetCode: (a: any) => [number, number, number, number];
    readonly search: (a: any, b: number, c: number, d: any, e: number) => [number, number, number];
    readonly shares: (a: number, b: number) => [number, number, number, number];
    readonly state_boardStatus: (a: number, b: any) => [number, number, number];
    readonly state_mark: (a: number, b: any) => [number, number, number];
    readonly state_ply: (a: number) => any;
    readonly state_ruleset: (a: number) => any;
    readonly state_status: (a: number) => any;
    readonly state_wonCount: (a: number, b: any) => [number, number, number];
    readonly __wbindgen_malloc: (a: number, b: number) => number;
    readonly __wbindgen_realloc: (a: number, b: number, c: number, d: number) => number;
    readonly __wbindgen_exn_store: (a: number) => void;
    readonly __externref_table_alloc: () => number;
    readonly __wbindgen_externrefs: WebAssembly.Table;
    readonly __externref_table_dealloc: (a: number) => void;
    readonly __externref_drop_slice: (a: number, b: number) => void;
    readonly __wbindgen_free: (a: number, b: number, c: number) => void;
    readonly __wbindgen_start: () => void;
}

export type SyncInitInput = BufferSource | WebAssembly.Module;

/**
 * Instantiates the given `module`, which can either be bytes or
 * a precompiled `WebAssembly.Module`.
 *
 * @param {{ module: SyncInitInput }} module - Passing `SyncInitInput` directly is deprecated.
 *
 * @returns {InitOutput}
 */
export function initSync(module: { module: SyncInitInput } | SyncInitInput): InitOutput;

/**
 * If `module_or_path` is {RequestInfo} or {URL}, makes a request and
 * for everything else, calls `WebAssembly.instantiate` directly.
 *
 * @param {{ module_or_path: InitInput | Promise<InitInput> }} module_or_path - Passing `InitInput` directly is deprecated.
 *
 * @returns {Promise<InitOutput>}
 */
export default function __wbg_init (module_or_path?: { module_or_path: InitInput | Promise<InitInput> } | InitInput | Promise<InitInput>): Promise<InitOutput>;
