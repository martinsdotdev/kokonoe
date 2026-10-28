import { describe, expect, it } from 'vitest';
import { openDB } from 'idb';
import {
	fromVersion1,
	latestUnfinished,
	loadGame,
	recentGames,
	saveGame,
	type SavedGame,
	type ViewVersion1
} from './saved.js';
import { SHELF } from './shelf.js';
import { position } from './view.fixtures.js';
import { at } from './view.js';

/** A fresh database for each test, so they share nothing. */
const fresh = () => `kokonoe-test-${crypto.randomUUID()}`;

const game = (id: string, updated: number, result: string | null = null): SavedGame => ({
	id,
	record: { rules_version: 1, ruleset: SHELF[0].ruleset, seed: '00000000005eed00', moves: [40] },
	variant: { name: 'Standard', code: 'std' },
	opponent: { kind: 'computer', level: 3, seat: 'X' },
	resigned: null,
	started: updated,
	updated,
	view: position((view) => {
		view.cells[40] = 'X';
		view.sent = [4];
		at(view, [4]).active = true;
		view.last = 40;
	}),
	toMove: result ? null : 'O',
	result
});

describe('saved games in IndexedDB', () => {
	it('keeps a game, and a save replaces it', async () => {
		const db = fresh();
		expect(await loadGame('a', db)).toBeUndefined();
		await saveGame(game('a', 1), db);
		expect(await loadGame('a', db)).toEqual(game('a', 1));
		await saveGame(game('a', 2, 'X wins on a line'), db);
		expect(await loadGame('a', db)).toEqual(game('a', 2, 'X wins on a line'));
		expect(await recentGames(10, db)).toHaveLength(1);
	});

	it('lists the most recently played first, up to the limit', async () => {
		const db = fresh();
		for (const [id, updated] of [
			['a', 3],
			['b', 1],
			['c', 4],
			['d', 2]
		] as const) {
			await saveGame(game(id, updated), db);
		}
		expect((await recentGames(10, db)).map((saved) => saved.id)).toEqual(['c', 'a', 'd', 'b']);
		expect((await recentGames(2, db)).map((saved) => saved.id)).toEqual(['c', 'a']);
	});

	it('continues the latest unfinished game, skipping finished ones', async () => {
		const db = fresh();
		expect(await latestUnfinished(db)).toBeUndefined();
		await saveGame(game('old', 1), db);
		await saveGame(game('done', 2, 'Draw'), db);
		expect((await latestUnfinished(db))?.id).toBe('old');
		await saveGame(game('new', 3), db);
		expect((await latestUnfinished(db))?.id).toBe('new');
	});

	it('reads a game version 1 saved, its view moved onto Board paths', async () => {
		const db = fresh();
		const old = await openDB(db, 1, {
			upgrade(raw) {
				raw.createObjectStore('games', { keyPath: 'id' }).createIndex('updated', 'updated');
			}
		});
		const { view, ...rest } = game('a', 1);
		const stored: ViewVersion1 = {
			continuation: false,
			cells: view.cells,
			locals: [...Array(9).keys()].map((space) => ({ space, status: 'Open', active: space === 4 })),
			sent: 4,
			free: false,
			last: 40
		};
		await old.put('games', { ...rest, view: stored });
		old.close();
		expect(await loadGame('a', db)).toEqual(game('a', 1));
		expect(fromVersion1({ ...stored, sent: 0, free: true })).toMatchObject({
			sent: [0],
			within: []
		});
	});

	it('reads a game version 2 saved, with no dealt Cells', async () => {
		const db = fresh();
		const old = await openDB(db, 2, {
			upgrade(raw) {
				raw.createObjectStore('games', { keyPath: 'id' }).createIndex('updated', 'updated');
			}
		});
		const { view, ...rest } = game('a', 1);
		const { dealt, ...stored } = view;
		expect(dealt).toEqual([]);
		await old.put('games', { ...rest, view: stored });
		old.close();
		expect(await loadGame('a', db)).toEqual(game('a', 1));
	});
});
