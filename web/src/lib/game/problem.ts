/**
 * Opening a game page: the saved game and the engine, loaded together, or a Problem that says
 * in words why the page can't open, for the player.
 */
import { loadEngine } from '#lib/engine.js';
import { loadGame, type SavedGame } from './saved.js';

/** A failure the page explains to the player, rather than only logging it. */
export class Problem extends Error {}

/** Throws `message` for the page to show, with the cause logged where a developer can see it. */
export const failed = (message: string) => (error: unknown) => {
	console.error(message, error);
	throw new Problem(message);
};

export const ENGINE = 'The engine didn’t load. Reload the page to try again.';

/** The saved game `id`, with the engine loaded alongside it. */
export const loadSaved = async (id: string): Promise<SavedGame> => {
	const [saved] = await Promise.all([
		loadGame(id).catch(failed('Saved games can’t be read in this browser.')),
		loadEngine().catch(failed(ENGINE))
	]);
	if (!saved) throw new Problem('This game isn’t saved on this device.');
	return saved;
};

/** What the page says about `error`: a Problem's own words, or else to reload, logged as `what`. */
export const explain = (error: unknown, what: string): string => {
	if (error instanceof Problem) return error.message;
	console.error(what, error);
	return 'Something went wrong. Reload the page to try again.';
};
