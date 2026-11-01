/** A refused move, as both kinds of game show it: the Board its refusal names flashes. */
import type { Board, IllegalMove } from '#lib/engine.js';

/** How long an illegal tap flashes the Board it names. */
export const FLASH_MS = 700;

/** The Board an IllegalMove names, if it names one other than the Global board. */
export const named = (illegal: IllegalMove): Board | null => {
	if (typeof illegal === 'string') return null;
	const board: Board =
		'BeneathDecided' in illegal ? illegal.BeneathDecided.board : illegal.OutsideActiveBoards.within;
	return board.length ? board : null;
};
