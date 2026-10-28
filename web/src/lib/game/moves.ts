/**
 * The move list and the player bars, as the game screen and the Replay viewer show them. Plain
 * data read from board views, with no engine in it.
 */
import type { Cell, Player } from '#lib/engine.js';
import type { Opponent } from './launch.js';
import { boardLabel, notation } from './notation.js';
import { at, lineage, localOf, type BoardView } from './view.js';

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

/** The rows of a dealt opening, which X begins and which do nothing worth a note. */
export const dealtMoves = (opening: Cell[], depth: 2 | 3): Move[] =>
	opening.map((cell, i) => ({
		n: i + 1,
		player: i % 2 ? 'O' : 'X',
		notation: notation(cell, depth),
		dealt: true,
		note: ''
	}));

/** What `cell` did, read from the board before and after it: the outermost Board it decided. */
export const note = (before: BoardView, after: BoardView, cell: Cell): string => {
	const decided = lineage(localOf(cell, after.depth))
		.slice(1)
		.find((board) => at(before, board).status === 'Open' && at(after, board).status !== 'Open');
	if (decided) {
		const label = boardLabel(decided);
		return at(after, decided).status === 'Drawn' ? `Board ${label} drawn` : `Won board ${label}`;
	}
	if (after.within) return 'Free move';
	return after.sent === null ? '' : `Sent → ${boardLabel(after.sent)}`;
};

/** What a player bar says about a seat. */
export interface Seat {
	name: string;
	/** "Level 3" for the computer. */
	detail: string | null;
}

export const seats = (opponent: Opponent): Record<Player, Seat> => {
	if (opponent.kind === 'hotseat') {
		return { X: { name: 'Player 1', detail: null }, O: { name: 'Player 2', detail: null } };
	}
	const you: Seat = { name: 'You', detail: null };
	const computer: Seat = { name: 'Computer', detail: `Level ${opponent.level}` };
	return opponent.seat === 'X' ? { X: you, O: computer } : { X: computer, O: you };
};
