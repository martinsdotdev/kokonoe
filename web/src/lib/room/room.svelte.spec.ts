import { beforeAll, describe, expect, it, vi } from 'vitest';
import { loadEngine, type ClientMsg } from '#lib/engine.js';
import { TOP_ROW, cells } from '#lib/game/games.fixtures.js';
import { UnplayableGame } from '#lib/game/record.js';
import { RoomGame } from './room.svelte.js';
import { FakeStore, ROOM, record, welcome } from './room.fixtures.js';

beforeAll(() => loadEngine());

const top = cells(TOP_ROW);

/** A Room's game whose link the test watches. */
const game = (...args: Parameters<typeof welcome>) => {
	const sent: ClientMsg[] = [];
	const link = {
		send: vi.fn((message: ClientMsg) => (sent.push(message), true)),
		resync: vi.fn()
	};
	const store = new FakeStore();
	return { room: new RoomGame(welcome(...args), link, { room: ROOM, store }), sent, link, store };
};

describe('a Room’s game', () => {
	it('shows what a Welcome holds', () => {
		const { room } = game('O', { X: 'guest-4821', O: 'guest-1066' }, top.slice(0, 3));
		expect(room.variant.name).toBe('Standard');
		expect(room.moves.map((move) => move.notation)).toEqual(['1·4', '4·1', '1·5']);
		expect(room.seats).toEqual({
			X: { name: 'guest-4821', detail: null, yours: false },
			O: { name: 'guest-1066', detail: null, yours: true }
		});
		expect(room.against).toBe('vs guest-4821');
		expect(room.toMove).toBe('O');
		expect(room.frozen).toBe(false);
		expect(room.controls).toBeNull();
	});

	it('sends a legal move and waits for the server before showing it', () => {
		const { room, sent } = game('X');
		room.play(top[0]);
		room.play(top[0]);
		expect(sent).toEqual([{ type: 'Move', ply: 0, cell: top[0] }]);
		expect(room.moves).toEqual([]);
		expect(room.frozen).toBe(true);

		room.receive({ type: 'Moved', ply: 0, cell: top[0] });
		expect(room.moves).toHaveLength(1);
		expect(room.pending).toBeNull();
		expect(room.replied).toBe(false);
		room.receive({ type: 'Moved', ply: 1, cell: top[1] });
		expect(room.replied, 'the other side’s move lands').toBe(true);
		expect(room.frozen).toBe(false);
	});

	it('refuses an illegal tap at once, sending nothing', () => {
		const { room, sent } = game('X', undefined, top.slice(0, 1));
		const { room: own } = game('O', undefined, top.slice(0, 1));
		own.play(top[0]);
		expect(own.refusal?.cell).toBe(top[0]);
		own.play(0);
		expect(own.flash?.board).toEqual([3]);
		room.play(top[1]);
		expect(sent, 'not X’s turn').toEqual([]);
	});

	it('resyncs on a gap, a refusal or a move it can’t play, and ignores one it has', () => {
		const { room, link } = game('X', undefined, top.slice(0, 2));
		room.receive({ type: 'Moved', ply: 1, cell: top[1] });
		expect(link.resync).not.toHaveBeenCalled();
		expect(room.moves).toHaveLength(2);
		room.receive({ type: 'Moved', ply: 3, cell: top[3] });
		room.receive({ type: 'Moved', ply: 2, cell: top[0] });
		room.receive({ type: 'Rejected', reason: 'NotAllowed' });
		expect(link.resync).toHaveBeenCalledTimes(3);
		expect(room.moves).toHaveLength(2);
	});

	it('waits for a fresh Welcome after a resync, taking no moves', () => {
		const { room, sent } = game('X');
		room.receive({ type: 'Rejected', reason: 'IllegalMove' });
		expect(room.frozen).toBe(true);
		room.play(top[0]);
		expect(sent).toEqual([]);
		room.welcome(welcome('X'));
		expect(room.frozen).toBe(false);
	});

	it('takes everything a later Welcome holds', () => {
		const { room } = game('X', { X: 'guest-4821', O: null });
		expect(room.seated).toBe(false);
		expect(room.frozen, 'waiting for a friend').toBe(true);
		room.welcome(welcome('X', { X: 'guest-4821', O: 'guest-1066' }, top.slice(0, 2)));
		expect(room.seated).toBe(true);
		expect(room.moves).toHaveLength(2);
		expect(room.frozen).toBe(false);
	});

	it('checks the server’s end against its own', () => {
		const { room, link } = game('X', undefined, top);
		expect(room.end).toEqual({ outcome: { Objective: 'X' } });
		room.receive({ type: 'Ended', result: '1-0', termination: 'line' });
		expect(link.resync).not.toHaveBeenCalled();
		room.receive({ type: 'Ended', result: '1/2-1/2', termination: 'agreed draw' });
		expect(link.resync).toHaveBeenCalledOnce();
	});

	it('says when the connection is lost, and takes no moves meanwhile', () => {
		const { room, sent } = game('X');
		room.dropped();
		expect(room.trouble).toEqual({ message: 'Connection lost. Reconnecting…' });
		room.play(top[0]);
		expect(sent).toEqual([]);
	});

	it('keeps itself after every move once both seats are filled', async () => {
		const waiting = game('X', { X: 'guest-4821', O: null });
		expect(waiting.store.saves, 'nothing to keep while waiting').toEqual([]);

		const { room, store } = game('X');
		expect(store.saves, 'nor before a move').toEqual([]);
		room.receive({ type: 'Moved', ply: 0, cell: top[0] });
		await expect.poll(() => store.saves.length).toBe(1);
		expect(store.last).toMatchObject({
			id: ROOM,
			record: { moves: [top[0]] },
			variant: { name: 'Standard', code: 'std' },
			opponent: {
				kind: 'room',
				room: ROOM,
				you: 'X',
				players: { X: 'guest-4821', O: 'guest-1066' }
			},
			toMove: 'O',
			result: null
		});
		await expect.poll(() => room.replay).toBe(ROOM);
	});

	it('keeps each move it plays, and nothing it doesn’t', async () => {
		const { room, store } = game('X');
		room.receive({ type: 'Moved', ply: 0, cell: top[0] });
		room.receive({ type: 'Moved', ply: 1, cell: top[1] });
		await expect
			.poll(() => store.saves.map((saved) => saved.record.moves))
			.toEqual([[top[0]], [top[0], top[1]]]);
		room.receive({ type: 'Moved', ply: 0, cell: top[0] });
		room.receive({ type: 'Moved', ply: 5, cell: top[5] });
		room.welcome(welcome('X', undefined, top.slice(0, 2)));
		expect(store.saves, 'a resend, a gap and a Welcome changing nothing').toHaveLength(2);
		room.welcome(welcome('X', undefined, top.slice(0, 3)));
		await expect.poll(() => store.saves.length).toBe(3);
	});

	it('names the Handles alone in its game file', () => {
		const { room } = game('O', undefined, top.slice(0, 2));
		expect(room.file.seats).toEqual({ X: 'guest-4821', O: 'guest-1066' });
		expect(room.seats.O).toMatchObject({ name: 'guest-1066', yours: true });
	});

	it('keeps a finished game with its result, for its Replay', async () => {
		const { store } = game('O', undefined, top);
		await expect.poll(() => store.saves.length).toBe(1);
		expect(store.last).toMatchObject({ toMove: null, result: 'X wins on a line' });
	});

	it('refuses a Record from another rules version', () => {
		const future = {
			...welcome('X'),
			snapshot: { ...welcome('X').snapshot, record: { ...record(), rules_version: 2 } }
		};
		const link = { send: () => true, resync: () => {} };
		expect(() => new RoomGame(future, link, { room: ROOM, store: new FakeStore() })).toThrow(
			UnplayableGame
		);
	});
});
