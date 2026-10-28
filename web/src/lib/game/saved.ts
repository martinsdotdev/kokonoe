/**
 * Offline games in IndexedDB (spec §5.1 and §14): every game from its first move, finished or
 * not. Plain data with no engine in it, so Home can list and preview games without loading the
 * WASM. A Record carries its own `rules_version`; the database's version covers only its shape.
 */
import { openDB, type DBSchema, type IDBPDatabase } from 'idb';
import type { BoardStatus, Cell, Player, Record } from '#lib/engine.js';
import type { Opponent } from './launch.js';
import type { Variant } from './rules.js';
import type { BoardView } from './view.js';

/** One offline game as stored. */
export interface SavedGame {
	/** Also the game's address, `/play?game=<id>`, and its Replay's, `/replay?game=<id>`. */
	id: string;
	record: Record;
	variant: { name: string; code: string };
	opponent: Opponent;
	/** Who resigned, which a Record can't hold. */
	resigned: Player | null;
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

interface Schema extends DBSchema {
	games: { key: string; value: SavedGame; indexes: { updated: number } };
}

export const DATABASE = 'kokonoe';
/**
 * Version 2 names Boards in the stored view by path, which depth 3 needs (KOK-39). Version 3
 * lists the view's dealt Cells (KOK-45).
 */
const VERSION = 3;

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

export const browserStore: GameStore = { save: (game) => saveGame(game) };
