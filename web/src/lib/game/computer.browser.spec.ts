import { afterEach, beforeAll, describe, expect, it } from 'vitest';
import { Game, loadEngine, newSeed, presets, type Record } from '#lib/engine.js';
import { WorkerComputer } from './computer.js';

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

describe('the computer in its Worker', () => {
	it('answers with a legal move', async () => {
		const cell = await computer.think(record, 3, new AbortController().signal);
		expect(legal).toContain(cell);
	});

	it('rejects when aborted, and answers the next request from a new Worker', async () => {
		const asking = new AbortController();
		const aborted = computer.think(record, 8, asking.signal);
		asking.abort();
		await expect(aborted).rejects.toThrow(/abort/i);
		const cell = await computer.think(record, 1, new AbortController().signal);
		expect(legal).toContain(cell);
	});

	it('passes on the engine’s error', async () => {
		const replayed = { ...record, moves: [0, 0] };
		await expect(computer.think(replayed, 3, new AbortController().signal)).rejects.toThrow(
			/move 1 is illegal/
		);
	});
});
