import { beforeAll, expect, it } from 'vitest';
import { Game, loadEngine, presets, type Cell } from '#lib/engine.js';
import { boardView } from './view.js';

let standard: () => Game;

beforeAll(async () => {
	await loadEngine();
	standard = () => new Game(presets()[0].ruleset, '00000000005eed00');
});

/** Plays Cells written as engine digit pairs: `"42"` is Space 4, then Space 2. */
const play = (game: Game, moves: string) => {
	for (const digits of moves.split(' ')) {
		const cell: Cell = Number(digits[0]) * 9 + Number(digits[1]);
		expect(game.play(game.ply(), cell)).toHaveProperty('Now');
	}
	return game;
};

it('reads the first move as no Sent target, every board Active', () => {
	const view = boardView(standard());
	expect(view.sent).toBeNull();
	expect(view.last).toBeNull();
	expect(view.locals.every((local) => local.active)).toBe(true);
});

it('reads marks, the Sent target and the last move', () => {
	const view = boardView(play(standard(), '42 24'));
	expect(view.cells[38]).toBe('X');
	expect(view.cells[22]).toBe('O');
	expect(view.last).toBe(22);
	expect(view.sent).toBe(4);
	expect(view.free).toBe(false);
	expect(view.locals.filter((local) => local.active).map((local) => local.space)).toEqual([4]);
});

it('reads a Free move after a send to a decided board', () => {
	const view = boardView(play(standard(), '03 30 04 40 05 50'));
	expect(view.locals[0].status).toEqual({ Won: 'X' });
	expect(view.sent).toBe(0);
	expect(view.free).toBe(true);
	expect(view.locals[0].active).toBe(false);
	expect(view.locals.filter((local) => local.active)).toHaveLength(8);
});
