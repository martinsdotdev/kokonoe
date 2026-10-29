/** Opening a saved Record with the engine, or saying in words why it can't be. */
import { Game, type Record, type ReplayError } from '#lib/engine.js';

/** A saved game this version of Kokonoe can't play on or replay: its message says why. */
export class UnplayableGame extends Error {}

export const illegalMove = (index: number) =>
	new UnplayableGame(`Its move ${index + 1} isn’t legal.`);

/**
 * The Game `record` leads to. Throws an UnplayableGame when this version of Kokonoe can't read
 * the Record, or one of its moves isn't legal.
 */
export const openRecord = (record: Record): Game => {
	// Only `open` checks the Record's rules version.
	let opened: Game | ReplayError;
	try {
		opened = Game.open(record);
	} catch {
		throw new UnplayableGame('It was saved in a form this version of Kokonoe can’t read.');
	}
	if (!(opened instanceof Game)) throw illegalMove(opened.index);
	return opened;
};
