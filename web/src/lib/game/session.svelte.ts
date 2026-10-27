import {
	Game,
	newSeed,
	type Board,
	type Cell,
	type IllegalMove,
	type Player,
	type Ruleset,
	type Status
} from '#lib/engine.js';
import type { Opponent } from './launch.js';
import { boardLabel, notation } from './notation.js';
import type { End } from './result.js';
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
 * goes on. Until KOK-41 the computer's turns are played by hand.
 */
export class OfflineGame {
	readonly variant: Variant;
	readonly opponent: Opponent;
	readonly seats: Record<Player, Seat>;
	#game: Game;
	#flashTimer: ReturnType<typeof setTimeout> | undefined;

	view: BoardView;
	status: Status;
	moves: Move[];
	won: Record<Player, number>;
	resigned: Player | null = $state.raw(null);
	/** The Local board to flash, with a tick so the same board can flash twice running. */
	flash: { space: number; tick: number } | null = $state.raw(null);

	constructor(variant: Variant, opponent: Opponent) {
		this.variant = variant;
		this.opponent = opponent;
		this.seats = seats(opponent);
		this.#game = new Game(variant.ruleset, newSeed());
		this.view = $state.raw(boardView(this.#game));
		this.status = $state.raw(this.#game.status());
		this.moves = $state.raw(dealt(this.#game));
		this.won = $state.raw(this.#counts());
	}

	get end(): End | null {
		if (this.resigned) return { resigned: this.resigned };
		return 'Over' in this.status ? { outcome: this.status.Over } : null;
	}

	get toMove(): Player | null {
		return 'ToMove' in this.status ? this.status.ToMove.player : null;
	}

	get canUndo(): boolean {
		return this.end === null && this.moves.some((move) => !move.dealt);
	}

	/** Plays `cell` for the player to move, or flashes the Board the refusal names. */
	play(cell: Cell) {
		const player = this.toMove;
		if (this.end || !player) return;
		const before = this.view;
		const result = this.#game.play(this.#game.ply(), cell);
		if (typeof result === 'string') return;
		if ('Now' in result) {
			this.#refresh();
			const move = { n: this.moves.length + 1, player, notation: notation(cell, 2), dealt: false };
			this.moves = [...this.moves, { ...move, note: note(before, this.view, cell) }];
		} else if ('Illegal' in result) {
			this.#flash(named(result.Illegal));
		}
	}

	/** Takes back the last move, never the dealt opening, and never after the game ends. */
	undo() {
		if (!this.canUndo) return;
		this.#game.undo();
		this.moves = this.moves.slice(0, -1);
		this.#refresh();
	}

	/** The player to move gives up. */
	resign() {
		if (!this.end && this.toMove) this.resigned = this.toMove;
	}

	/** A fresh game of the same variant and opponent, with a new Seed. */
	restart() {
		this.#game.free();
		this.#game = new Game(this.variant.ruleset, newSeed());
		this.resigned = null;
		this.moves = dealt(this.#game);
		this.#flash(null);
		this.#refresh();
	}

	/** Frees the engine's memory. */
	dispose() {
		clearTimeout(this.#flashTimer);
		this.#game.free();
	}

	#refresh() {
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
