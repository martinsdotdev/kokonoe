import {
	Game,
	newSeed,
	type Board,
	type Cell,
	type IllegalMove,
	type Player,
	type ReplayError,
	type Ruleset,
	type Status
} from '#lib/engine.js';
import { WorkerComputer, type Computer } from './computer.js';
import type { Level, Opponent } from './launch.js';
import { boardLabel, notation } from './notation.js';
import { resultText, type End } from './result.js';
import { rulesetCode } from './rules.js';
import { browserStore, type GameStore, type SavedGame } from './saved.js';
import { boardView, type BoardView } from './view.js';

/** One row of the move list. */
export interface Move {
	/** The ply, counted from 1, the dealt opening included. */
	n: number;
	player: Player;
	notation: string;
	/** Dealt by a Random opening rather than played. */
	dealt: boolean;
	/** What the move did: "Sent → 4", "Won board 3", "Board 8 drawn", "Free move". */
	note: string;
}

/** How long an illegal tap flashes the Board it names. */
const FLASH_MS = 700;

/** The computer never moves sooner than this after the player (spec §6). */
const PACE_MS = 500;

/** A Preset or Custom variant: the rules and what to call them. */
export interface Variant {
	name: string;
	ruleset: Ruleset;
}

/** What a player bar says about a seat. */
export interface Seat {
	name: string;
	/** "Level 3" for the computer. */
	detail: string | null;
}

const seats = (opponent: Opponent): Record<Player, Seat> => {
	if (opponent.kind === 'hotseat') {
		return { X: { name: 'Player 1', detail: null }, O: { name: 'Player 2', detail: null } };
	}
	const you: Seat = { name: 'You', detail: null };
	const computer: Seat = { name: 'Computer', detail: `Level ${opponent.level}` };
	return opponent.seat === 'X' ? { X: you, O: computer } : { X: computer, O: you };
};

/** The Local board an IllegalMove names, if it names one. */
const named = (illegal: IllegalMove): number | null => {
	if (typeof illegal === 'string') return null;
	const board: Board =
		'BeneathDecided' in illegal ? illegal.BeneathDecided.board : illegal.OutsideActiveBoards.within;
	return board.length === 1 ? board[0] : null;
};

/** What `cell` did, read from the board before and after it. */
const note = (before: BoardView, after: BoardView, cell: Cell): string => {
	const space = Math.floor(cell / 9);
	const board = boardLabel([space]);
	const now = after.locals[space].status;
	if (before.locals[space].status === 'Open' && now !== 'Open') {
		return now === 'Drawn' ? `Board ${board} drawn` : `Won board ${board}`;
	}
	if (after.free) return 'Free move';
	return after.sent === null ? '' : `Sent → ${boardLabel([after.sent])}`;
};

/** Resolves after `ms`, or rejects when `signal` aborts first. */
const delay = (ms: number, signal: AbortSignal) =>
	new Promise<void>((resolve, reject) => {
		const timer = setTimeout(resolve, ms);
		signal.addEventListener(
			'abort',
			() => {
				clearTimeout(timer);
				reject(signal.reason);
			},
			{ once: true }
		);
	});

export interface Options {
	/** The computer, for a game against it: a Web Worker unless a test brings its own. */
	computer?: Computer;
	/** The least time between the player's move and the computer's, in milliseconds. */
	pace?: number;
	/** Where the game keeps itself: IndexedDB unless a test brings its own. */
	store?: GameStore;
}

/** A saved game that can't be played on: its message says why, for the player. */
export class UnplayableGame extends Error {}

const illegalMove = (index: number) => new UnplayableGame(`Its move ${index + 1} isn’t legal.`);

const dealt = (game: Game): Move[] =>
	game.opening().map((cell, i) => ({
		n: i + 1,
		player: i % 2 ? 'O' : 'X',
		notation: notation(cell, 2),
		dealt: true,
		note: ''
	}));

/**
 * A game on this device: passed between two players, or against the computer. It owns the
 * engine's Game and is the one writer of what the screen shows: each action changes the engine,
 * then reads it back. A finished game stays finished: Undo and Resign stop, and only `restart`
 * goes on.
 *
 * Against the computer it asks for a move whenever the computer is to move, and plays the answer
 * no sooner than the pace allows. Undo, Resign, `restart` and `dispose` abort a pending request,
 * so an answer that arrives later plays nothing.
 *
 * From its first move, by either side, it saves itself after every change, so `resume` can
 * rebuild it after a reload.
 */
export class OfflineGame {
	readonly variant: Variant;
	readonly opponent: Opponent;
	readonly seats: Record<Player, Seat>;
	/** The player's seat against the computer, null when passing and playing. */
	readonly human: Player | null;
	#game: Game;
	#store: GameStore;
	/** When the game was first saved, in epoch milliseconds: null until its first move. */
	#started: number | null = null;
	/** The computer and its Level, in a game against it. */
	#against: { computer: Computer; level: Level } | null;
	#pace: number;
	/** The pending request to the computer, to abort. */
	#asking: AbortController | null = null;
	/** What the computer is doing: nothing, searching for its move, or stuck after a failure. */
	#phase: 'idle' | 'thinking' | 'stuck' = $state.raw('idle');
	#flashTimer: ReturnType<typeof setTimeout> | undefined;

	view: BoardView;
	status: Status;
	moves: Move[];
	won: Record<Player, number>;
	resigned: Player | null = $state.raw(null);
	/** The Local board to flash, with a tick so the same board can flash twice running. */
	flash: { space: number; tick: number } | null = $state.raw(null);
	/** The last change was the computer's move, which the phone board shows landing. */
	replied = $state.raw(false);
	/** The game's id in the store, and its address once `stored`. A restart gives a new one. */
	id: string = $state.raw(crypto.randomUUID());
	/** A save of the game has succeeded, so `id` can be its address. */
	stored = $state.raw(false);

	/**
	 * Continues a saved game: its Record replayed move by move, so the move list has its notes.
	 * Throws an UnplayableGame when this version of Kokonoe can't play the Record on.
	 */
	static resume(saved: SavedGame, options: Options = {}): OfflineGame {
		// Only `open` checks the Record's rules version, so check with it before replaying.
		let opened: Game | ReplayError;
		try {
			opened = Game.open(saved.record);
		} catch {
			throw new UnplayableGame('It was saved in a form this version of Kokonoe can’t read.');
		}
		if (!(opened instanceof Game)) throw illegalMove(opened.index);
		opened.free();
		const variant = { name: saved.variant.name, ruleset: saved.record.ruleset };
		return new OfflineGame(variant, saved.opponent, options, saved);
	}

	/** A new game, or with `saved`, the one `resume` continues. */
	constructor(variant: Variant, opponent: Opponent, options: Options = {}, saved?: SavedGame) {
		this.variant = variant;
		this.opponent = opponent;
		this.seats = seats(opponent);
		this.human = opponent.kind === 'computer' ? opponent.seat : null;
		this.#against =
			opponent.kind === 'computer'
				? { computer: options.computer ?? new WorkerComputer(), level: opponent.level }
				: null;
		this.#pace = options.pace ?? PACE_MS;
		this.#store = options.store ?? browserStore;
		this.#game = new Game(variant.ruleset, saved?.record.seed ?? newSeed());
		this.view = $state.raw(boardView(this.#game));
		this.status = $state.raw(this.#game.status());
		this.moves = $state.raw(dealt(this.#game));
		this.won = $state.raw(this.#counts());
		if (saved) {
			for (const cell of saved.record.moves) {
				if (this.#apply(cell)) continue;
				this.dispose();
				throw illegalMove(this.moves.filter((move) => !move.dealt).length);
			}
			this.id = saved.id;
			this.resigned = saved.resigned;
			this.stored = true;
			this.#started = saved.started;
		}
		this.#ask();
	}

	get end(): End | null {
		if (this.resigned) return { resigned: this.resigned };
		return 'Over' in this.status ? { outcome: this.status.Over } : null;
	}

	get toMove(): Player | null {
		return 'ToMove' in this.status ? this.status.ToMove.player : null;
	}

	/** The computer is searching for its move. */
	get thinking(): boolean {
		return this.#phase === 'thinking';
	}

	/** The computer's last request failed, and `retry` asks again. */
	get stuck(): boolean {
		return this.#phase === 'stuck';
	}

	/** It's the computer's turn, so the Board takes no taps. */
	get computerToMove(): boolean {
		return this.human !== null && this.toMove !== null && this.toMove !== this.human;
	}

	/** Who Resign gives up for: the player to move, or the player against the computer. */
	get resigner(): Player | null {
		return this.end ? null : (this.human ?? this.toMove);
	}

	/** Undo needs one of the player's moves: against the computer, its own don't count. */
	get canUndo(): boolean {
		return (
			this.end === null &&
			this.moves.some((move) => !move.dealt && (this.human === null || move.player === this.human))
		);
	}

	/** Plays `cell` for the player to move, or flashes the Board the refusal names. */
	play(cell: Cell) {
		if (!this.computerToMove) this.#play(cell);
	}

	/**
	 * Takes back the last move, never the dealt opening, and never after the game ends. Against
	 * the computer it takes back the computer's reply with the player's move, or stops the search
	 * and takes back the player's move.
	 */
	undo() {
		if (!this.canUndo) return;
		this.#cancel();
		let undone: Move | undefined;
		do {
			undone = this.moves.at(-1);
			this.#game.undo();
			this.moves = this.moves.slice(0, -1);
		} while (this.human && undone?.player !== this.human);
		this.#refresh();
		this.#save();
	}

	/** The player to move gives up, or against the computer the player, even on its turn. */
	resign() {
		const player = this.resigner;
		if (!player) return;
		this.#cancel();
		this.resigned = player;
		this.#save();
	}

	/** A fresh game of the same variant and opponent, with a new Seed and a new id. */
	restart() {
		this.#cancel();
		this.#game.free();
		this.#game = new Game(this.variant.ruleset, newSeed());
		this.id = crypto.randomUUID();
		this.stored = false;
		this.#started = null;
		this.resigned = null;
		this.moves = dealt(this.#game);
		this.#flash(null);
		this.#refresh();
		this.#ask();
	}

	/** Asks the computer again after a failed request. */
	retry() {
		if (this.stuck) this.#ask();
	}

	/** Stops the computer and frees the engine's memory. */
	dispose() {
		this.#cancel();
		this.#against?.computer.dispose();
		clearTimeout(this.#flashTimer);
		this.#game.free();
	}

	#play(cell: Cell, replied = false) {
		if (!this.#apply(cell)) return;
		this.replied = replied;
		this.#save();
		this.#ask();
	}

	/** Plays `cell` for the side to move and lists it, or flashes the Board a refusal names. */
	#apply(cell: Cell): boolean {
		const player = this.toMove;
		if (this.end || !player) return false;
		const before = this.view;
		const result = this.#game.play(this.#game.ply(), cell);
		if (typeof result === 'string') return false;
		if ('Illegal' in result) this.#flash(named(result.Illegal));
		if (!('Now' in result)) return false;
		this.#refresh();
		this.moves = [
			...this.moves,
			{
				n: this.moves.length + 1,
				player,
				notation: notation(cell, 2),
				dealt: false,
				note: note(before, this.view, cell)
			}
		];
		return true;
	}

	/**
	 * Saves the game, from its first move on. `stored` waits for the first save to succeed, so
	 * the address never names a game the store doesn't hold. A failed save costs play nothing.
	 */
	#save() {
		if (this.#started === null && this.moves.every((move) => move.dealt)) return;
		const updated = Date.now();
		this.#started ??= updated;
		const end = this.end;
		const saved: SavedGame = {
			id: this.id,
			record: this.#game.record(),
			variant: { name: this.variant.name, code: rulesetCode(this.variant.ruleset) },
			opponent: this.opponent,
			resigned: this.resigned,
			started: this.#started,
			updated,
			view: this.view,
			toMove: end ? null : this.toMove,
			result: end ? resultText(end, this.variant.ruleset, this.won) : null
		};
		this.#store.save(saved).then(
			() => {
				if (this.id === saved.id) this.stored = true;
			},
			(error: unknown) => console.error('The game couldn’t be saved.', error)
		);
	}

	/** Asks the computer for its move when it's to move, and plays the answer. */
	async #ask() {
		const against = this.#against;
		if (!against || this.end || !this.computerToMove) return;
		const asking = new AbortController();
		this.#asking = asking;
		this.#phase = 'thinking';
		try {
			const [cell] = await Promise.all([
				against.computer.think(this.#game.record(), against.level, asking.signal),
				delay(this.#pace, asking.signal)
			]);
			if (this.#asking !== asking) return;
			this.#settle('idle');
			this.#play(cell, true);
		} catch (error) {
			if (this.#asking !== asking) return;
			// The screen only says it failed, so say why where a developer can see it.
			console.error('The computer couldn’t move.', error);
			this.#settle('stuck');
		}
	}

	/** Aborts a pending request, so its answer plays nothing whenever it comes. */
	#cancel() {
		this.#asking?.abort();
		this.#settle('idle');
	}

	#settle(phase: 'idle' | 'stuck') {
		this.#asking = null;
		this.#phase = phase;
	}

	#refresh() {
		this.replied = false;
		this.view = boardView(this.#game);
		this.status = this.#game.status();
		this.won = this.#counts();
	}

	#counts(): Record<Player, number> {
		return { X: this.#game.wonCount('X'), O: this.#game.wonCount('O') };
	}

	#flash(space: number | null) {
		clearTimeout(this.#flashTimer);
		this.flash = space === null ? null : { space, tick: (this.flash?.tick ?? 0) + 1 };
		if (space !== null) this.#flashTimer = setTimeout(() => (this.flash = null), FLASH_MS);
	}
}
