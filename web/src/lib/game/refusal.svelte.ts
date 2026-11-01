/**
 * A refused move, as both kinds of game show it: the Board its refusal names flashes for a
 * moment, and the refusal itself lasts until the game next changes, so the turn line says it once.
 */
import type { Board, Cell, IllegalMove } from '#lib/engine.js';

/** How long an illegal tap flashes the Board it names. */
const FLASH_MS = 700;

/** The Board to flash, with a tick so the same board can flash twice running. */
export type Flash = { board: Board; tick: number };

/** The refused Cell, the Board its refusal named, if any, and a tick for each refusal. */
export type Refusal = { cell: Cell; board: Board | null; tick: number };

/** The Board an IllegalMove names, if it names one other than the Global board. */
const named = (illegal: IllegalMove): Board | null => {
	if (typeof illegal === 'string') return null;
	const board: Board =
		'BeneathDecided' in illegal ? illegal.BeneathDecided.board : illegal.OutsideActiveBoards.within;
	return board.length ? board : null;
};

export class Refusals {
	flash: Flash | null = $state.raw(null);
	refusal: Refusal | null = $state.raw(null);
	#timer: ReturnType<typeof setTimeout> | undefined;

	/** `cell` was refused, for `illegal`. */
	refuse(cell: Cell, illegal: IllegalMove) {
		const board = named(illegal);
		clearTimeout(this.#timer);
		this.flash = board === null ? null : { board, tick: (this.flash?.tick ?? 0) + 1 };
		if (board !== null) this.#timer = setTimeout(() => (this.flash = null), FLASH_MS);
		this.refusal = { cell, board, tick: (this.refusal?.tick ?? 0) + 1 };
	}

	/** The game changed, so the refusal is said and done. */
	clear() {
		this.refusal = null;
	}

	/** Nothing flashes or stands refused: a new game, or the end of this one. */
	dispose() {
		clearTimeout(this.#timer);
		this.flash = null;
		this.refusal = null;
	}
}
