/**
 * A store for tests: it keeps every save, cloned the way IndexedDB clones it, reads them back, and
 * forgets a game it's told to delete.
 */
import type { ReadingStore, SavedGame } from './saved.js';

export class FakeStore implements ReadingStore {
	saves: SavedGame[] = [];
	/** The ids deleted, in order. */
	deleted: string[] = [];

	/** The latest save; a test that reads it expects one. */
	get last(): SavedGame {
		const last = this.saves.at(-1);
		if (!last) throw new Error('Nothing was saved.');
		return last;
	}

	async save(game: SavedGame) {
		this.saves.push(structuredClone(game));
	}

	async load(id: string) {
		return this.saves.findLast((game) => game.id === id);
	}

	async delete(id: string) {
		this.deleted.push(id);
		this.saves = this.saves.filter((game) => game.id !== id);
	}
}
