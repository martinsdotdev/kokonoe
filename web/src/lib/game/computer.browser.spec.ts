import { afterEach, beforeAll, describe, expect, it } from 'vitest';
import { Game, loadEngine, newSeed, presets, type Record } from '#lib/engine.js';
import { WorkerComputer } from './computer.js';
import { TOP_ROW, cells } from './games.fixtures.js';

let record: Record;
let legal: number[];

beforeAll(async () => {
	await loadEngine();
	const game = new Game(presets()[0].ruleset, newSeed());
	game.play(0, 40);
	const status = game.status();
	record = game.record();
	legal = 'ToMove' in status ? status.ToMove.moves : [];
	game.free();
});

const computer = new WorkerComputer();
afterEach(() => computer.dispose());

const asking = () => new AbortController().signal;

/** Asks an hour ahead, so the Level's cap never binds on a busy test machine. */
const later = () => Date.now() + 3_600_000;

describe('the computer in its Workers', () => {
	it('answers with a legal move, from the Level’s whole budget', async () => {
		const patient = new WorkerComputer({ now: later });
		try {
			expect(legal).toContain(await patient.think(record, 3, asking()));
			expect(patient.searched).toBe(300);
		} finally {
			patient.dispose();
		}
	});

	it('searches a Level’s whole budget across one to four Workers', async () => {
		for (const workers of [1, 2, 3, 4]) {
			const split = new WorkerComputer({ workers, now: later });
			try {
				const cell = await split.think(record, 5, asking());
				expect(legal).toContain(cell);
				expect(split.searched, `${workers} Workers`).toBe(3_000);
			} finally {
				split.dispose();
			}
		}
	});

	it('moves by the Level’s cap, counted from when the page asked, however slow the device', async () => {
		// Asked so long ago that only 300 ms of Level 8's 9 s cap at depth 3 is left.
		const deep = new Game(presets()[4].ruleset, newSeed());
		const opening = deep.record();
		deep.free();
		const hurried = new WorkerComputer({ workers: 4, now: () => Date.now() - 8_700 });
		try {
			const start = performance.now();
			await hurried.think(opening, 8, asking());
			const took = performance.now() - start;
			expect(took, 'the cap, plus starting four Workers and choosing').toBeLessThan(300 + 1_500);
			expect(hurried.searched).toBeLessThan(100_000);
		} finally {
			hurried.dispose();
		}
	});

	it('plays a forced move at once, without a search', async () => {
		// X can complete the top row with 3·9.
		const almost = { ...record, moves: cells(TOP_ROW).slice(0, -1) };
		const split = new WorkerComputer({ workers: 4 });
		try {
			expect(await split.think(almost, 8, asking())).toBe(cells('28')[0]);
			expect(split.searched).toBe(0);
		} finally {
			split.dispose();
		}
	});

	it('rejects when aborted, and answers the next request from new Workers', async () => {
		const stopping = new AbortController();
		const aborted = computer.think(record, 8, stopping.signal);
		stopping.abort();
		await expect(aborted).rejects.toThrow(/abort/i);
		const cell = await computer.think(record, 1, asking());
		expect(legal).toContain(cell);
	});

	it('passes on the engine’s error', async () => {
		const replayed = { ...record, moves: [0, 0] };
		await expect(computer.think(replayed, 3, asking())).rejects.toThrow(/move 1 is illegal/);
	});
});
