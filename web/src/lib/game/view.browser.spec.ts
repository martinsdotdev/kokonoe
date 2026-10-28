import { beforeAll, describe, expect, it } from 'vitest';
import { Game, loadEngine, presets } from '#lib/engine.js';
import { INTO_A_WON_BOARD, cells } from './games.fixtures.js';
import { at, boardView, children, positionView, type BoardView } from './view.js';

let standard: () => Game;
let deep: () => Game;

beforeAll(async () => {
	await loadEngine();
	standard = () => new Game(presets()[0].ruleset, '00000000005eed00');
	deep = () => new Game(presets()[4].ruleset, '00000000005eed00');
});

/** Plays moves written as the engine's tests write them: `"42"` is Space 4, then Space 2. */
const play = (game: Game, moves: string) => {
	for (const cell of cells(moves)) expect(game.play(game.ply(), cell)).toHaveProperty('Now');
	return game;
};

/** The Active children of `board`. */
const active = (view: BoardView, board: number[]) =>
	children(board).filter((child) => at(view, child).active);

it('reads the first move as no Sent target, every board Active', () => {
	const view = boardView(standard());
	expect(view.sent).toBeNull();
	expect(view.last).toBeNull();
	expect(view.depth).toBe(2);
	expect(view.within).toBeNull();
	expect(view.boards).toHaveLength(1);
	expect(view.boards[0].every((board) => board.active)).toBe(true);
});

it('reads marks, the Sent target and the last move', () => {
	const view = boardView(play(standard(), '42 24'));
	expect(view.cells[38]).toBe('X');
	expect(view.cells[22]).toBe('O');
	expect(view.last).toBe(22);
	expect(view.sent).toEqual([4]);
	expect(view.within).toBeNull();
	expect(active(view, [])).toEqual([[4]]);
});

it('reads a Free move after a send to a decided board', () => {
	const view = boardView(play(standard(), '03 30 04 40 05 50'));
	expect(at(view, [0]).status).toEqual({ Won: 'X' });
	expect(view.sent).toEqual([0]);
	expect(view.within).toEqual([]);
	expect(at(view, [0]).active).toBe(false);
	expect(active(view, [])).toHaveLength(8);
});

describe('a Random opening', () => {
	const dealt = () =>
		new Game({ ...presets()[0].ruleset, opening: { Random: 4 } }, '00000000005eed00');

	it('lists the dealt Cells apart from the moves played', () => {
		const game = dealt();
		const opening = game.opening();
		expect(boardView(game).dealt).toEqual(opening);
		const view = boardView(play(game, '03'));
		expect(view.dealt).toEqual(opening);
		expect(view.last).toBe(cells('03')[0]);
	});

	it('reads any ply, partway through the deal', () => {
		const game = play(dealt(), '03');
		const plies = [...game.opening(), ...game.moves()];
		const state = game.stateAt(2)!;
		const view = positionView(state, plies[1], plies.slice(0, 2));
		state.free();
		expect(view.dealt).toEqual(plies.slice(0, 2));
		expect(view.last).toBe(plies[1]);
		expect(plies.map((cell) => view.cells[cell])).toEqual(['X', 'O', null, null, null]);
		expect(view.sent).not.toBeNull();
	});
});

describe('Deep', () => {
	it('reads the first move as every Board Active, at both levels', () => {
		const view = boardView(deep());
		expect(view.depth).toBe(3);
		expect(view.cells).toHaveLength(729);
		expect(view.boards.map((level) => level.length)).toEqual([9, 81]);
		expect(view.boards.flat().every((board) => board.active)).toBe(true);
	});

	it('reads the Sent target as a Local board’s path, and only its Boards as Active', () => {
		const view = boardView(play(deep(), '407'));
		expect(view.sent).toEqual([0, 7]);
		expect(view.within).toBeNull();
		expect(active(view, [])).toEqual([[0]]);
		expect(active(view, [0])).toEqual([[0, 7]]);
	});

	it('reads a Free move inside a middle Board, after a send to a won Local board', () => {
		const view = boardView(play(deep(), INTO_A_WON_BOARD));
		expect(view.sent).toEqual([7, 6]);
		expect(view.within).toEqual([7]);
		expect(at(view, [7, 6])).toEqual({ status: { Won: 'X' }, active: false });
		expect(at(view, [7])).toEqual({ status: 'Open', active: true });
		expect(active(view, [])).toEqual([[7]]);
		expect(active(view, [7])).toHaveLength(8);
	});
});
