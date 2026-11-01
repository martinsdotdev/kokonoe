import { beforeAll, describe, expect, it, vi } from 'vitest';
import { loadEngine, type ClientMsg } from '#lib/engine.js';
import { TOP_ROW, cells } from '#lib/game/games.fixtures.js';
import { UnplayableGame } from '#lib/game/record.js';
import { RoomGame } from './room.svelte.js';
import { record, welcome } from './room.fixtures.js';

beforeAll(() => loadEngine());

const top = cells(TOP_ROW);

/** A Room's game whose link the test watches. */
const game = (...args: Parameters<typeof welcome>) => {
	const sent: ClientMsg[] = [];
	const link = {
		send: vi.fn((message: ClientMsg) => (sent.push(message), true)),
		resync: vi.fn()
	};
	return { room: new RoomGame(welcome(...args), link), sent, link };
};

describe('a Room’s game', () => {
	it('shows what a Welcome holds', () => {
		const { room } = game('O', { X: 'guest-4821', O: 'guest-1066' }, top.slice(0, 3));
		expect(room.variant.name).toBe('Standard');
		expect(room.moves.map((move) => move.notation)).toEqual(['1·4', '4·1', '1·5']);
		expect(room.seats).toEqual({
			X: { name: 'guest-4821', detail: null },
			O: { name: 'guest-1066', detail: 'You' }
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
		room.receive({ type: 'Ended', outcome: { Objective: 'X' } });
		expect(link.resync).not.toHaveBeenCalled();
		room.receive({ type: 'Ended', outcome: 'Draw' });
		expect(link.resync).toHaveBeenCalledOnce();
	});

	it('says when the connection is lost, and takes no moves meanwhile', () => {
		const { room, sent } = game('X');
		room.dropped();
		expect(room.trouble).toEqual({ message: 'Connection lost. Reconnecting…' });
		room.play(top[0]);
		expect(sent).toEqual([]);
	});

	it('refuses a Record from another rules version', () => {
		const future = {
			...welcome('X'),
			snapshot: { ...welcome('X').snapshot, record: { ...record(), rules_version: 2 } }
		};
		const link = { send: () => true, resync: () => {} };
		expect(() => new RoomGame(future, link)).toThrow(UnplayableGame);
	});
});
