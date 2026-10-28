import type { Board, BoardStatus, Cell, Game, Player } from '#lib/engine.js';
import { cellPath, pathOf } from './notation.js';

/** A Board under the Global board as drawn: a Local board, or at depth 3 a middle Board. */
export interface BoardState {
	status: BoardStatus;
	/** A legal Cell lies beneath it this turn. */
	active: boolean;
}

/**
 * Everything the board draws, as plain data. The board renders only this, so a test can build
 * any position by hand, including ones the engine can't reach yet. Boards go by their path of
 * Spaces from the Global board, as the engine names them.
 */
export interface BoardView {
	depth: 2 | 3;
	/** Won Boards keep taking moves and show their owner as a watermark. */
	continuation: boolean;
	/** Each Cell's mark, by Cell index: 81 of them, or 729 at depth 3. */
	cells: (Player | null)[];
	/**
	 * The Boards under the Global board a level at a time, each by its path read in base 9: the
	 * Global board's nine children, then at depth 3 the 81 Local boards. Read them with `at`.
	 */
	boards: BoardState[][];
	/** The Sent target, a Local board, or null on the first move and once the game is over. */
	sent: Board | null;
	/** Under a Free move, the Board the player may play anywhere inside; otherwise null. */
	within: Board | null;
	last: Cell | null;
}

const range = (n: number) => [...Array(n).keys()];

/** A path read as a number in base 9: its place among the Boards of its level. */
const indexOf = (board: Board) => board.reduce((index, space) => index * 9 + space, 0);

/** What the view holds for `board`, any Board but the Global board. */
export const at = (view: BoardView, board: Board): BoardState =>
	view.boards[board.length - 1][indexOf(board)];

/** A Board's nine children, by Space. */
export const children = (board: Board): Board[] => range(9).map((space) => [...board, space]);

/** A Local board's nine Cells, by Space. */
export const cellsOf = (local: Board): Cell[] => children(local).map(indexOf);

/** The Local board `cell` lies in. */
export const localOf = (cell: Cell, depth: 2 | 3): Board => cellPath(cell, depth).slice(0, -1);

/** Whether two paths name the same Board. Null names none, so it matches nothing. */
export const same = (a: Board | null | undefined, b: Board | null | undefined): boolean =>
	a != null && b != null && a.length === b.length && a.every((space, i) => space === b[i]);

/** A path as its Spaces' digits, for keys and attributes: `'4'`, `'47'`, or `''` for Global. */
export const key = (board: Board): string => board.join('');

/** The player who won a Board, or null. */
export const ownerOf = (board: BoardState): Player | null =>
	typeof board.status === 'object' ? board.status.Won : null;

export const boardView = (game: Game): BoardView => {
	const depth = game.ruleset().depth;
	const status = game.status();
	const moves = 'ToMove' in status ? status.ToMove.moves : [];
	const sent = 'ToMove' in status ? status.ToMove.sent : 'FirstMove';
	return {
		depth,
		continuation: game.ruleset().continuation === 'On',
		cells: range(9 ** depth).map((cell) => game.mark(cell) ?? null),
		boards: range(depth - 1).map((level) => {
			// A Board's Cells are one run of indices, as long as 9 to the levels below it.
			const length = level + 1;
			const run = 9 ** (depth - length);
			const active = new Set(moves.map((cell) => Math.floor(cell / run)));
			return range(9 ** length).map((index) => ({
				status: game.boardStatus(pathOf(index, length)),
				active: active.has(index)
			}));
		}),
		sent: sent === 'FirstMove' ? null : 'To' in sent ? sent.To : sent.Free.target,
		within: sent !== 'FirstMove' && 'Free' in sent ? sent.Free.within : null,
		last: [...game.opening(), ...game.moves()].at(-1) ?? null
	};
};
