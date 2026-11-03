/**
 * Saved games in IndexedDB (spec §5.1 and §14): every game on this device from its first move,
 * finished or not, and this page's copy of each invite Room it played, kept after every move. Plain
 * data with no engine in it, so Home can list and preview games without loading the WASM. A Record
 * carries its own `rules_version`; the database's version covers only its shape.
 */
import { openDB, type DBSchema, type IDBPDatabase } from 'idb';
import type { BoardStatus, Cell, Player, Record, TimeControl } from '#lib/engine.js';
import type { SavedOpponent } from './launch.js';
import { resultText, type End, type Stopped } from './result.js';
import { rulesetCode, type Variant } from './rules.js';
import type { BoardView } from './view.js';

/** One saved game. */
export interface SavedGame {
	/**
	 * Also its Replay's address, `/replay?game=<id>`, and for a game on this device its own,
	 * `/play?game=<id>`. A Room's game is kept by its Room's id, and resumes at `/r/<room>`.
	 */
	id: string;
	record: Record;
	variant: { name: string; code: string };
	opponent: SavedOpponent;
	/** Who resigned, which a Record can't hold. */
	resigned: Player | null;
	/** The players agreed a draw, which a Record can't hold either. Absent from older games. */
	agreed?: true;
	/** Who ran out of time, in a timed Room. Absent from older games. */
	flagged?: Player;
	/** Who claimed the win against a player who left the Room. Absent from older games. */
	claimed?: Player;
	/** A draw was claimed against a player who left the Room. Absent from older games. */
	claimedDraw?: true;
	/** Both players left the Room, and it ended with no result. Absent from older games. */
	abandoned?: true;
	/** A timed Room's time control. Absent from untimed games. */
	clock?: TimeControl;
	/** When the game began and was last played, in epoch milliseconds. */
	started: number;
	updated: number;
	/** The position, so Home can draw it without the engine. */
	view: BoardView;
	/** Null once the game is over. */
	toMove: Player | null;
	/** The result banner's words, or null while the game goes on. */
	result: string | null;
}

/** Where a game keeps itself: IndexedDB, or a fake in tests. */
export interface GameStore {
	save(game: SavedGame): Promise<void>;
}

/** A store a game is also read back from, and deleted from. */
export interface ReadingStore extends GameStore {
	load(id: string): Promise<SavedGame | undefined>;
	delete(id: string): Promise<void>;
}

/** A game where it stands, as saved: what Home shows of it, worked out without the engine. */
export const savedGame = (game: {
	id: string;
	record: Record;
	variant: Variant;
	opponent: SavedOpponent;
	started: number;
	view: BoardView;
	toMove: Player | null;
	end: End | null;
	won: { X: number; O: number };
	clock?: TimeControl | null;
}): SavedGame => {
	// Each way a game stops is one field, named as its key.
	const stopped = game.end && !('outcome' in game.end) ? game.end : null;
	return {
		id: game.id,
		record: game.record,
		variant: { name: game.variant.name, code: rulesetCode(game.variant.ruleset) },
		opponent: game.opponent,
		resigned: null,
		...stopped,
		...(game.clock ? { clock: game.clock } : {}),
		started: game.started,
		updated: Date.now(),
		view: game.view,
		toMove: game.end ? null : game.toMove,
		result: game.end ? resultText(game.end, game.variant.ruleset, game.won) : null
	};
};

/** How a saved game stopped, if it did before its rules ended it. */
export const stoppedOf = (saved: SavedGame): Stopped | null => {
	if (saved.resigned) return { resigned: saved.resigned };
	if (saved.agreed) return { agreed: true };
	if (saved.flagged) return { flagged: saved.flagged };
	if (saved.claimed) return { claimed: saved.claimed };
	if (saved.claimedDraw) return { claimedDraw: true };
	return saved.abandoned ? { abandoned: true } : null;
};

interface Schema extends DBSchema {
	games: { key: string; value: SavedGame; indexes: { updated: number } };
}

export const DATABASE = 'kokonoe';
/**
 * Version 2 names Boards in the stored view by path, which depth 3 needs (KOK-39). Version 3
 * lists the view's dealt Cells (KOK-45). Version 4 keeps invite Rooms' games (KOK-56): it rewrites
 * nothing, but a build that doesn't know a `room` opponent mustn't open a store holding one, and
 * would otherwise resume it as pass and play.
 */
const VERSION = 4;

/** A view as version 1 stored it: depth 2, each Local board by its Space. */
export interface ViewVersion1 {
	continuation: boolean;
	cells: (Player | null)[];
	locals: { space: number; status: BoardStatus; active: boolean }[];
	sent: number | null;
	free: boolean;
	last: Cell | null;
}

/** A version-1 view as version 3 stores it. Every game was depth 2 then. */
export const fromVersion1 = (old: ViewVersion1): BoardView => ({
	depth: 2,
	continuation: old.continuation,
	cells: old.cells,
	boards: [old.locals.map(({ status, active }) => ({ status, active }))],
	sent: old.sent === null ? null : [old.sent],
	within: old.free ? [] : null,
	last: old.last,
	dealt: []
});

/**
 * A version-2 view as version 3 stores it. Which Cells were dealt needs the engine to deal them
 * again, so they're drawn as played until the game is next saved.
 */
export const fromVersion2 = (old: Omit<BoardView, 'dealt'>): BoardView => ({ ...old, dealt: [] });

const databases = new Map<string, Promise<IDBPDatabase<Schema>>>();

/** One connection per database, reopened after another tab's upgrade closes it. */
const database = (name: string): Promise<IDBPDatabase<Schema>> => {
	const known = databases.get(name);
	if (known) return known;
	const forget = () => {
		if (databases.get(name) === opening) databases.delete(name);
	};
	const opening = openDB<Schema>(name, VERSION, {
		// Only IndexedDB requests may be awaited here, or the upgrade's transaction closes.
		async upgrade(db, from, _to, transaction) {
			if (from < 1) {
				db.createObjectStore('games', { keyPath: 'id' }).createIndex('updated', 'updated');
				return;
			}
			if (from < 3) {
				let cursor = await transaction.objectStore('games').openCursor();
				while (cursor) {
					const old: unknown = cursor.value.view;
					const view =
						from < 2
							? fromVersion1(old as ViewVersion1)
							: fromVersion2(old as Omit<BoardView, 'dealt'>);
					await cursor.update({ ...cursor.value, view });
					cursor = await cursor.continue();
				}
			}
		},
		// An older build's tab still holds the store: the upgrade waits until it closes.
		blocked() {
			console.warn('Saved games wait for another Kokonoe tab to close.');
		},
		// A newer build wants to change the store: step aside so its upgrade isn't blocked.
		blocking() {
			forget();
			void opening.then((db) => db.close());
		},
		terminated: forget
	});
	// A failed open isn't kept, so the next call tries again.
	opening.catch(forget);
	databases.set(name, opening);
	return opening;
};

export const saveGame = async (game: SavedGame, name = DATABASE): Promise<void> => {
	await (await database(name)).put('games', game);
};

export const loadGame = async (id: string, name = DATABASE): Promise<SavedGame | undefined> =>
	(await database(name)).get('games', id);

export const deleteGame = async (id: string, name = DATABASE): Promise<void> => {
	await (await database(name)).delete('games', id);
};

/** The games played most recently, newest first, as many as `keep` accepts up to `limit`. */
const latest = async (
	name: string,
	limit: number,
	keep: (game: SavedGame) => boolean
): Promise<SavedGame[]> => {
	const games: SavedGame[] = [];
	const index = (await database(name)).transaction('games').store.index('updated');
	let cursor = await index.openCursor(null, 'prev');
	while (cursor && games.length < limit) {
		if (keep(cursor.value)) games.push(cursor.value);
		cursor = await cursor.continue();
	}
	return games;
};

export const recentGames = (limit: number, name = DATABASE): Promise<SavedGame[]> =>
	latest(name, limit, () => true);

/** The unfinished game played most recently, for Home's "Continue game". */
export const latestUnfinished = async (name = DATABASE): Promise<SavedGame | undefined> =>
	(await latest(name, 1, (game) => game.result === null))[0];

/** The variant a saved game plays, its rules read from its Record. */
export const savedVariant = (saved: SavedGame): Variant => ({
	name: saved.variant.name,
	ruleset: saved.record.ruleset
});

export const browserStore: ReadingStore = {
	save: (game) => saveGame(game),
	load: (id) => loadGame(id),
	delete: (id) => deleteGame(id)
};
