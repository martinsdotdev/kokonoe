import { afterEach, beforeAll, describe, expect, it, vi } from 'vitest';
import { loadEngine } from '#lib/engine.js';
import { DEALT, LINE_OF_MIDDLE_BOARDS, TOP_ROW, cells } from './games.fixtures.js';
import { UnplayableGame } from './record.js';
import { fromSaved, Replay } from './replay.svelte.js';
import type { GameStore, SavedGame } from './saved.js';
import { OfflineGame } from './session.svelte.js';
import { SHELF } from './shelf.js';
import { position } from './view.fixtures.js';
import { at } from './view.js';

beforeAll(() => loadEngine());

afterEach(() => {
	vi.useRealTimers();
});

const DEEP = SHELF.find((preset) => preset.name === 'Deep')!.ruleset;

/** A finished game as the store keeps it: what the Replay viewer opens. */
const saved = (record: Partial<SavedGame['record']> = {}): SavedGame => ({
	id: 'a',
	record: {
		rules_version: 1,
		ruleset: { ...DEEP, opening: { Random: 2 } },
		seed: DEALT.seed,
		moves: cells(LINE_OF_MIDDLE_BOARDS),
		...record
	},
	variant: { name: 'Custom variant', code: DEALT.code },
	opponent: { kind: 'hotseat' },
	resigned: null,
	started: 0,
	updated: 0,
	view: position(() => {}, 3),
	toMove: null,
	result: 'X wins on a line'
});

const store: GameStore = { save: async () => {} };

describe('a replayed game', () => {
	it('has a position for every ply, from the empty board to the end', () => {
		const replay = Replay.open(fromSaved(saved()));
		expect(replay.last).toBe(55);
		expect(replay.moments).toHaveLength(56);
		expect(replay.moments[0].view.cells.every((mark) => mark === null)).toBe(true);
		expect(replay.moments[0].view.last).toBeNull();
		expect(replay.end).toEqual({ outcome: { Objective: 'X' } });
		const end = replay.moments[55];
		for (const space of [0, 1, 2]) expect(at(end.view, [space]).status).toEqual({ Won: 'X' });
	});

	it('marks the dealt opening', () => {
		const replay = Replay.open(fromSaved(saved()));
		const dealt = cells(DEALT.opening);
		expect(replay.moves.slice(0, 3).map((move) => move.dealt)).toEqual([true, true, false]);
		expect(replay.moments[1].view.dealt).toEqual(dealt.slice(0, 1));
		expect(replay.moments[2].view.dealt).toEqual(dealt);
		expect(replay.moments[55].view.dealt).toEqual(dealt);
	});

	it('lists the moves and draws the end as the game screen does', () => {
		const replay = Replay.open(fromSaved(saved()));
		const resumed = OfflineGame.resume(saved(), { store });
		expect(replay.moves).toEqual(resumed.moves);
		expect(replay.moments[55].view).toEqual(resumed.view);
		const middle = replay.moves.filter((move) => /^Won board \d$/.test(move.note));
		expect(middle.map((move) => move.note).sort()).toEqual([
			'Won board 1',
			'Won board 2',
			'Won board 3'
		]);
		resumed.dispose();
	});

	it('says what each ply did', () => {
		const replay = Replay.open(fromSaved(saved()));
		expect(replay.caption).toBe('Start. X moves first.');
		replay.go(2);
		expect(replay.caption).toBe('Move 2 of 55 · O (1·3·9) · dealt');
		replay.go(3);
		expect(replay.caption).toBe('Move 3 of 55 · X 3·9·1 · Sent → 9·1');
	});

	it('steps only between the start and the end', () => {
		const replay = Replay.open(fromSaved(saved()));
		replay.go(-1);
		expect(replay.ply).toBe(0);
		replay.go(99);
		expect(replay.ply).toBe(55);
		expect(replay.shown).toBe(replay.moments[55]);
	});

	it('autoplays once a second, stops at the end, and starts again from the start', () => {
		vi.useFakeTimers();
		const replay = Replay.open(
			fromSaved(saved({ ruleset: SHELF[0].ruleset, moves: cells(TOP_ROW) }))
		);
		replay.toggle();
		expect(replay.playing).toBe(true);
		vi.advanceTimersByTime(2000);
		expect(replay.ply).toBe(2);
		replay.go(5);
		expect(replay.playing, 'a jump stops it').toBe(false);
		vi.advanceTimersByTime(2000);
		expect(replay.ply).toBe(5);

		replay.toggle();
		vi.advanceTimersByTime(60_000);
		expect(replay.ply).toBe(17);
		expect(replay.playing).toBe(false);
		replay.toggle();
		expect(replay.ply).toBe(0);
		vi.advanceTimersByTime(1000);
		expect(replay.ply).toBe(1);
		replay.dispose();
	});

	it('refuses a Record it can’t replay', () => {
		expect(() => Replay.open(fromSaved(saved({ moves: [0, 0] })))).toThrow(UnplayableGame);
		expect(() => Replay.open(fromSaved(saved({ rules_version: 2 })))).toThrow(UnplayableGame);
	});
});
