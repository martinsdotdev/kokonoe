import { beforeAll, describe, expect, it } from 'vitest';
import { loadEngine, type Cell } from '#lib/engine.js';
import { HotSeat } from './session.svelte.js';

beforeAll(() => loadEngine());

/** Cells written as engine digit pairs: `"42"` is Space 4, then Space 2. */
const cells = (moves: string): Cell[] =>
	moves.split(' ').map((digits) => Number(digits[0]) * 9 + Number(digits[1]));

/** X takes Local boards 0, 1 and 2, each with a row, while O sends X wherever it needs to go. */
const TOP_ROW = '03 30 04 40 05 51 13 31 14 41 15 52 26 62 27 72 28';

const after = (moves: string) => {
	const session = new HotSeat();
	for (const cell of cells(moves)) session.play(cell);
	return session;
};

describe('a hot-seat game', () => {
	it('plays to a line win, and then stays finished', () => {
		const session = after(TOP_ROW);
		expect(session.end).toEqual({ outcome: { Objective: 'X' } });
		expect(session.moves).toHaveLength(17);
		expect(session.canUndo).toBe(false);
		session.undo();
		session.resign();
		session.play(80);
		expect(session.moves).toHaveLength(17);
		expect(session.resigned).toBeNull();
	});

	it('notes what each move did', () => {
		const session = after('03 30 04 40 05 50');
		expect(session.moves.map((move) => [move.player, move.notation, move.note])).toEqual([
			['X', '1·4', 'Sent → 4'],
			['O', '4·1', 'Sent → 1'],
			['X', '1·5', 'Sent → 5'],
			['O', '5·1', 'Sent → 1'],
			['X', '1·6', 'Won board 1'],
			['O', '6·1', 'Free move']
		]);
	});

	it('undoes back to the first move', () => {
		const session = after('42 24 40');
		while (session.canUndo) session.undo();
		expect(session.moves).toEqual([]);
		expect(session.view.cells.every((mark) => mark === null)).toBe(true);
		expect(session.status).toMatchObject({ ToMove: { player: 'X', sent: 'FirstMove' } });
	});

	it('flashes the Board an illegal move names, and plays nothing', () => {
		const session = after('42');
		session.play(0);
		expect(session.flash).toEqual({ space: 2, tick: 1 });
		session.play(0);
		expect(session.flash).toEqual({ space: 2, tick: 2 });
		session.play(cells('42')[0]);
		expect(session.flash, 'Occupied names no Board, so nothing flashes').toBeNull();
		expect(session.moves).toHaveLength(1);
	});

	it('resigns for the player to move, and restarts', () => {
		const session = after('42');
		session.resign();
		expect(session.end).toEqual({ resigned: 'O' });
		session.restart();
		expect(session.end).toBeNull();
		expect(session.moves).toEqual([]);
		expect(session.toMove).toBe('X');
	});
});
