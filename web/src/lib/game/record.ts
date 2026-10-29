/** Opening a saved game with the engine, or saying in words why it can't be. */
import { Game, loadEngine, type Record, type ReplayError } from '#lib/engine.js';
import { ENGINE, failed, Problem } from './problem.js';
import { loadGame, type SavedGame } from './saved.js';

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

/** The saved game `id`, with the engine loaded alongside it. */
export const loadSaved = async (id: string): Promise<SavedGame> => {
	const [saved] = await Promise.all([
		loadGame(id).catch(failed('Saved games can’t be read in this browser.')),
		loadEngine().catch(failed(ENGINE))
	]);
	if (!saved) throw new Problem('This game isn’t saved on this device.');
	return saved;
};
