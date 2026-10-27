import type { BoardStatus, Cell, Game, Player } from '#lib/engine.js';

/** One Local board of a depth-2 game. */
export interface LocalView {
	/** Where it sits on the Global board, 0 to 8. */
	space: number;
	status: BoardStatus;
	/** It holds a legal Cell this turn. */
	active: boolean;
}

/**
 * Everything the board draws, as plain data. The board renders only this, so a test can build
 * any position by hand, including ones the engine can't reach yet. Depth 2: Deep's step-into view
 * is KOK-38 and KOK-39.
 */
export interface BoardView {
	/** Won Boards keep taking moves and show their owner as a watermark. */
	continuation: boolean;
	/** Each Cell's mark, by Cell index. */
	cells: (Player | null)[];
	/** The nine Local boards, by Space. */
	locals: LocalView[];
	/** The Space of the Sent target, or null on the first move and once the game is over. */
	sent: number | null;
	/** The Sent target can take no move, so every Active board is open. */
	free: boolean;
	last: Cell | null;
}

/** The player who won a Local board, or null. */
export const ownerOf = (local: LocalView): Player | null =>
	typeof local.status === 'object' ? local.status.Won : null;

const SPACES = [...Array(9).keys()];
const CELLS = [...Array(81).keys()];

export const boardView = (game: Game): BoardView => {
	const status = game.status();
	const sent = 'ToMove' in status ? status.ToMove.sent : null;
	const active = new Set(
		'ToMove' in status ? status.ToMove.moves.map((cell) => Math.floor(cell / 9)) : []
	);
	return {
		continuation: game.ruleset().continuation === 'On',
		cells: CELLS.map((cell) => game.mark(cell) ?? null),
		locals: SPACES.map((space) => ({
			space,
			status: game.boardStatus([space]),
			active: active.has(space)
		})),
		sent:
			sent === null || sent === 'FirstMove'
				? null
				: 'To' in sent
					? sent.To[0]
					: sent.Free.target[0],
		free: sent !== null && sent !== 'FirstMove' && 'Free' in sent,
		last: [...game.opening(), ...game.moves()].at(-1) ?? null
	};
};
