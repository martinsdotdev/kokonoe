/**
 * Notation for people (spec §4.1): a Cell's or Board's path of Spaces from the Global board down,
 * each Space plus 1, joined by `·`. `5·5` is the centre Cell of the centre Local board. Display
 * only: KOK-34 adds the engine's parser for text files.
 */
import type { Board, Cell } from '#lib/engine.js';

/** A Cell's Spaces from the Global board down, each 0 to 8. */
export const cellPath = (cell: Cell, depth: 2 | 3): number[] => {
	const path: number[] = [];
	for (let n = cell, i = 0; i < depth; i++, n = Math.floor(n / 9)) path.unshift(n % 9);
	return path;
};

const label = (path: readonly number[]) => path.map((space) => space + 1).join('·');

export const notation = (cell: Cell, depth: 2 | 3): string => label(cellPath(cell, depth));

/** `4`, or `1·8` at depth 3. The Global board, the empty path, has no label. */
export const boardLabel = (board: Board): string => label(board);
