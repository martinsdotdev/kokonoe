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

/** Who made ply `n`, counted from 1: X moves first and turns alternate, the deal included. */
export const playerAt = (n: number): Player => (n % 2 ? 'X' : 'O');

/** The rows of a dealt opening, which do nothing worth a note. */
export const dealtMoves = (opening: Cell[], depth: 2 | 3): Move[] =>
	opening.map((cell, i) => ({
		n: i + 1,
		player: playerAt(i + 1),
		notation: notation(cell, depth),
		dealt: true,
		note: ''
	}));

/** What `cell` did, read from the board before and after it: the outermost Board it decided. */
const note = (before: BoardView, after: BoardView, cell: Cell): string => {
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

/** The row of ply `n`, `cell`, read from the board `before` and `after` it. */
export const playedMove = (n: number, cell: Cell, before: BoardView, after: BoardView): Move => ({
	n,
	player: playerAt(n),
	notation: notation(cell, after.depth),
	dealt: false,
	note: note(before, after, cell)
});

/** A move as the list writes it: a dealt one in parentheses, `(5·5)`. */
export const written = (move: Move): string => (move.dealt ? `(${move.notation})` : move.notation);

/** How many moves the players made, the deal aside. */
export const playedCount = (moves: Move[]): number => moves.filter((move) => !move.dealt).length;

/** What a player bar says about a seat. */
export interface Seat {
	name: string;
	/** "Level 3" for the computer. */
	detail: string | null;
}

/** A seat in one line, as a game file names it: "Player 1", "Computer, Level 3". */
export const seatLabel = (seat: Seat): string =>
	seat.detail ? `${seat.name}, ${seat.detail}` : seat.name;

export const seats = (opponent: Opponent): Record<Player, Seat> => {
	if (opponent.kind === 'hotseat') {
		return { X: { name: 'Player 1', detail: null }, O: { name: 'Player 2', detail: null } };
	}
	const you: Seat = { name: 'You', detail: null };
	const computer: Seat = { name: 'Computer', detail: `Level ${opponent.level}` };
	return opponent.seat === 'X' ? { X: you, O: computer } : { X: computer, O: you };
};
