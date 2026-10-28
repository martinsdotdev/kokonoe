/**
 * Notation for people (spec §4.1): a Cell's or Board's path of Spaces from the Global board down,
 * each Space plus 1, joined by `·`. `5·5` is the centre Cell of the centre Local board. The engine
 * owns Notation; this copy formats it on pages that load no engine, and
 * `encodings.browser.spec.ts` holds it to the engine's for every Cell.
 */
import type { Board, Cell } from '#lib/engine.js';

/**
 * The path `length` Spaces long that `index` names, read in base 9 with the first Space first: a
 * Cell's Spaces from the Global board down, or a Board's.
 */
export const pathOf = (index: number, length: number): number[] => {
	const path: number[] = [];
	for (let n = index, i = 0; i < length; i++, n = Math.floor(n / 9)) path.unshift(n % 9);
	return path;
};

const label = (path: readonly number[]) => path.map((space) => space + 1).join('·');

export const notation = (cell: Cell, depth: 2 | 3): string => label(pathOf(cell, depth));

/** `4`, or `1·8` at depth 3. The Global board, the empty path, has no label. */
export const boardLabel = (board: Board): string => label(board);

/** A Board as copy names it: `Board 5`, or `Local board 5·8` two levels down at depth 3. */
export const boardName = (board: Board): string =>
	`${board.length === 2 ? 'Local board' : 'Board'} ${boardLabel(board)}`;
