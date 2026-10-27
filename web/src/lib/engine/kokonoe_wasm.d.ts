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
     * The game a Record leads to, or the first of its moves the rules reject.
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
 * The five Presets, Standard first.
 */
export function presets(): Preset[];

/**
 * The computer's move in the game `record` leads to, at `level` 1 to 8, searched from `seed`
 * (spec §6). For the AI Worker: one call per move, so the search never crosses into JavaScript
 * except to read `Date.now` against the Level's time cap. A Level out of range, a Record the
 * rules reject and a finished game all throw, since the game screen never asks for them.
 */
export function think(record: Record, level: number, seed: Seed): Cell;

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
    readonly memory: WebAssembly.Memory;
    readonly __wbg_game_free: (a: number, b: number) => void;
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
    readonly game_status: (a: number) => any;
    readonly game_undo: (a: number) => any;
    readonly game_wonCount: (a: number, b: any) => [number, number, number];
    readonly presets: () => [number, number];
    readonly think: (a: any, b: number, c: any) => [number, number, number];
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
