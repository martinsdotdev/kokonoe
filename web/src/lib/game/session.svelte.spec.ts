import { beforeAll, describe, expect, it, vi } from 'vitest';
import { loadEngine, type Cell, type Record } from '#lib/engine.js';
import type { Computer } from './computer.js';
import type { Opponent } from './launch.js';
import { OfflineGame } from './session.svelte.js';
import { SHELF } from './shelf.js';

beforeAll(() => loadEngine());

/** Cells written as engine digit pairs: `"42"` is Space 4, then Space 2. */
const cells = (moves: string): Cell[] =>
	moves.split(' ').map((digits) => Number(digits[0]) * 9 + Number(digits[1]));

/** X takes Local boards 0, 1 and 2, each with a row, while O sends X wherever it needs to go. */
const TOP_ROW = '03 30 04 40 05 51 13 31 14 41 15 52 26 62 27 72 28';

const after = (moves: string) => {
	const session = new OfflineGame(SHELF[0], { kind: 'hotseat' });
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

describe('the opponent and the variant', () => {
	it('names the seats for pass and play, and for the computer', () => {
		expect(new OfflineGame(SHELF[0], { kind: 'hotseat' }).seats).toEqual({
			X: { name: 'Player 1', detail: null },
			O: { name: 'Player 2', detail: null }
		});
		const computer = new OfflineGame(
			SHELF[0],
			{ kind: 'computer', level: 4, seat: 'O' },
			{ computer: new FakeComputer() }
		);
		expect(computer.seats).toEqual({
			X: { name: 'Computer', detail: 'Level 4' },
			O: { name: 'You', detail: null }
		});
	});

	it('plays the variant it was given, and restarts it', () => {
		const tictacku = SHELF.find((preset) => preset.name === 'Tic-Tac-Ku')!;
		const session = new OfflineGame(tictacku, { kind: 'hotseat' });
		for (const cell of cells('03 30 04 40 05')) session.play(cell);
		expect(session.won).toEqual({ X: 1, O: 0 });
		expect(session.end, 'first to 5, not 1').toBeNull();
		session.restart();
		expect(session.variant.name).toBe('Tic-Tac-Ku');
		expect(session.won).toEqual({ X: 0, O: 0 });
	});
});

/** A request to the computer, which the test answers by hand. */
interface Ask {
	record: Record;
	level: number;
	signal: AbortSignal;
	answer: (cell: Cell) => void;
	fail: (error: Error) => void;
}

class FakeComputer implements Computer {
	asks: Ask[] = [];
	disposed = false;

	think(record: Record, level: number, signal: AbortSignal): Promise<Cell> {
		return new Promise((answer, fail) => {
			this.asks.push({ record, level, signal, answer, fail });
			signal.addEventListener('abort', () => fail(signal.reason));
		});
	}

	dispose() {
		this.disposed = true;
	}
}

/** Lets an answer through the session's awaits and its zero pace. */
const settle = () => new Promise((resolve) => setTimeout(resolve, 0));

const against = (seat: 'X' | 'O', pace = 0) => {
	const computer = new FakeComputer();
	const opponent: Opponent = { kind: 'computer', level: 3, seat };
	return { computer, session: new OfflineGame(SHELF[0], opponent, { computer, pace }) };
};

const [C42, C24, C40, C03] = cells('42 24 40 03');

describe('a game against the computer', () => {
	it('answers the player, and the Board takes no taps on its turn', async () => {
		const { computer, session } = against('X');
		expect(computer.asks).toHaveLength(0);
		session.play(C42);
		expect(session.thinking).toBe(true);
		expect(computer.asks).toHaveLength(1);
		expect(computer.asks[0].level).toBe(3);
		expect(computer.asks[0].record.moves).toEqual([C42]);
		session.play(C24);
		expect(session.moves, 'not the player’s move to make').toHaveLength(1);
		computer.asks[0].answer(C24);
		await settle();
		expect(session.thinking).toBe(false);
		expect(session.moves.map((move) => [move.player, move.notation])).toEqual([
			['X', '5·3'],
			['O', '3·5']
		]);
		expect(session.toMove).toBe('X');
	});

	it('opens when it sits on X, and Undo waits for the player’s move', async () => {
		const { computer, session } = against('O');
		expect(computer.asks).toHaveLength(1);
		expect(session.canUndo).toBe(false);
		computer.asks[0].answer(C42);
		await settle();
		expect(session.moves).toHaveLength(1);
		expect(session.canUndo, 'only the computer has moved').toBe(false);
		session.play(C24);
		expect(session.canUndo).toBe(true);
	});

	it('undo takes back the reply with the player’s move', async () => {
		const { computer, session } = against('X');
		session.play(C42);
		computer.asks[0].answer(C24);
		await settle();
		session.undo();
		expect(session.moves).toEqual([]);
		expect(session.toMove).toBe('X');
		expect(session.canUndo).toBe(false);
	});

	it('undo during the search stops it, and its answer plays nothing', async () => {
		const { computer, session } = against('X');
		session.play(C42);
		session.undo();
		expect(computer.asks[0].signal.aborted).toBe(true);
		expect(session.thinking).toBe(false);
		expect(session.moves).toEqual([]);
		session.play(C40);
		computer.asks[0].answer(C03);
		await settle();
		expect(
			session.moves.map((move) => move.notation),
			'no stale reply'
		).toEqual(['5·1']);
		computer.asks[1].answer(C03);
		await settle();
		expect(session.moves.map((move) => move.notation)).toEqual(['5·1', '1·4']);
	});

	it('resigns for the player, even during the search', () => {
		const { computer, session } = against('X');
		session.play(C42);
		expect(session.resigner).toBe('X');
		session.resign();
		expect(session.end).toEqual({ resigned: 'X' });
		expect(computer.asks[0].signal.aborted).toBe(true);
		expect(session.resigner).toBeNull();
	});

	it('restarts without the pending answer, and asks again when it opens', async () => {
		const { computer, session } = against('O');
		session.restart();
		expect(computer.asks[0].signal.aborted).toBe(true);
		expect(computer.asks).toHaveLength(2);
		computer.asks[0].answer(C42);
		computer.asks[1].answer(C40);
		await settle();
		expect(session.moves.map((move) => move.notation)).toEqual(['5·1']);
	});

	it('says when it fails, and asks again on retry', async () => {
		const error = vi.spyOn(console, 'error').mockImplementation(() => {});
		const { computer, session } = against('X');
		session.play(C42);
		computer.asks[0].fail(new Error('the Worker crashed'));
		await settle();
		expect(session.stuck).toBe(true);
		expect(session.thinking).toBe(false);
		expect(error).toHaveBeenCalledOnce();
		session.retry();
		expect(session.stuck).toBe(false);
		computer.asks[1].answer(C24);
		await settle();
		expect(session.moves).toHaveLength(2);
		error.mockRestore();
	});

	it('waits out the pace before it moves', async () => {
		const { computer, session } = against('X', 100);
		session.play(C42);
		computer.asks[0].answer(C24);
		await settle();
		expect(session.moves, 'answered, but too soon to show').toHaveLength(1);
		await vi.waitFor(() => expect(session.moves).toHaveLength(2));
	});

	it('stops the computer when disposed', () => {
		const { computer, session } = against('X');
		session.play(C42);
		session.dispose();
		expect(computer.asks[0].signal.aborted).toBe(true);
		expect(computer.disposed).toBe(true);
	});
});
