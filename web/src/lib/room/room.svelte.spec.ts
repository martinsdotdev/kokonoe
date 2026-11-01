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
	const onrematch = vi.fn();
	const room = new RoomGame(welcome(...args), link, { room: ROOM, store, onrematch });
	return { room, sent, link, store, onrematch };
};

const NEXT = 'b'.repeat(20);

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
		expect(room.controls).toMatchObject({ resigner: 'O', offerDraw: { enabled: true } });
		expect(room.controls.askTakeback, 'a Room without takebacks').toBeUndefined();
		expect(room.offer).toBeNull();
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

	it('resigns, and keeps the resignation', async () => {
		const { room, sent, store } = game('X', undefined, top.slice(0, 2));
		room.controls.resign();
		expect(sent).toEqual([{ type: 'Resign' }]);
		room.receive({ type: 'Ended', result: '0-1', termination: 'resign' });
		expect(room.end).toEqual({ resigned: 'X' });
		expect(room.frozen).toBe(true);
		expect(room.controls.resigner).toBeNull();
		expect(room.file.end).toEqual({ resigned: 'X' });
		await expect.poll(() => store.last?.result).toBe('X resigned. O wins.');
		expect(store.last?.resigned).toBe('X');
	});

	it('offers a draw, shows it at once, and hears when it isn’t accepted', () => {
		const { room, sent } = game('X', undefined, top.slice(0, 2));
		room.controls.offerDraw?.run();
		expect(sent).toEqual([{ type: 'OfferDraw' }]);
		expect(room.offer).toEqual({ kind: 'Draw', mine: true, other: 'guest-1066', lapses: null });
		expect(room.controls.offerDraw?.enabled, 'one offer at a time').toBe(false);
		room.receive({ type: 'Offer', kind: 'Draw', by: 'X' });
		room.receive({ type: 'OfferAnswered', kind: 'Draw', accepted: false });
		expect(room.offer).toBeNull();
		expect(room.notice).toBe('guest-1066 didn’t accept the draw.');
		room.receive({ type: 'Moved', ply: 2, cell: top[2] });
		expect(room.notice, 'until the next move').toBeNull();
	});

	it('answers the other player’s draw, and ends agreed', async () => {
		const { room, sent, store } = game('X', undefined, top.slice(0, 2));
		room.receive({ type: 'Offer', kind: 'Draw', by: 'O' });
		expect(room.offer).toEqual({ kind: 'Draw', mine: false, other: 'guest-1066', lapses: null });
		room.answer(true);
		expect(sent).toEqual([{ type: 'AnswerDraw', accept: true }]);
		expect(room.offer).toBeNull();
		room.receive({ type: 'OfferAnswered', kind: 'Draw', accepted: true });
		room.receive({ type: 'Ended', result: '1/2-1/2', termination: 'agreed draw' });
		expect(room.end).toEqual({ agreed: true });
		expect(room.notice).toBeNull();
		await expect.poll(() => store.last?.result).toBe('Draw agreed');
		expect(store.last?.agreed).toBe(true);
	});

	it('counts each player’s draw offers, and allows none past the last', () => {
		const { room } = game('X', undefined, top.slice(0, 2), { draws: { X: 2, O: 3 } });
		room.receive({ type: 'Offer', kind: 'Draw', by: 'X' });
		room.receive({ type: 'OfferAnswered', kind: 'Draw', accepted: false });
		expect(room.controls.offerDraw?.enabled).toBe(false);
	});

	it('drops an offer a move withdraws, as the server does', () => {
		const { room } = game('X', undefined, top.slice(0, 2), { takebacks: true });
		room.receive({ type: 'Offer', kind: 'Draw', by: 'X' });
		room.receive({ type: 'Moved', ply: 2, cell: top[2] });
		room.receive({ type: 'OfferAnswered', kind: 'Draw', accepted: false });
		expect(room.offer).toBeNull();
		expect(room.notice, 'its own move withdrew it').toBeNull();

		room.receive({ type: 'Offer', kind: 'Draw', by: 'X' });
		room.receive({ type: 'Moved', ply: 3, cell: top[3] });
		expect(room.offer, 'a draw offer stands through the other’s move').toMatchObject({
			kind: 'Draw'
		});
		room.receive({ type: 'OfferAnswered', kind: 'Draw', accepted: false });

		room.receive({ type: 'Offer', kind: 'Takeback', by: 'O' });
		room.receive({ type: 'Moved', ply: 4, cell: top[4] });
		expect(room.offer, 'any move withdraws a takeback').toBeNull();
	});

	it('asks a takeback where the Room allows them, and takes the game back', async () => {
		const { room, sent, store } = game('O', undefined, top.slice(0, 2), { takebacks: true });
		expect(room.controls.askTakeback?.enabled).toBe(true);
		room.controls.askTakeback?.run();
		expect(sent).toEqual([{ type: 'AskTakeback' }]);
		const heard = Date.now();
		room.receive({ type: 'Offer', kind: 'Takeback', by: 'O' });
		expect(room.offer?.lapses).toBeGreaterThanOrEqual(heard + 15_000);
		room.receive({ type: 'OfferAnswered', kind: 'Takeback', accepted: true });
		room.receive({ type: 'TookBack', to_ply: 1 });
		expect(room.moves.map((move) => move.notation)).toEqual(['1·4']);
		expect(room.toMove).toBe('O');
		expect(room.offer).toBeNull();
		await expect.poll(() => store.last?.record.moves).toEqual([top[0]]);
	});

	it('needs a move of this player’s own to ask a takeback', () => {
		const { room } = game('O', undefined, top.slice(0, 1), { takebacks: true });
		expect(room.controls.askTakeback?.enabled).toBe(false);
	});

	it('keeps a takeback to before every move, so nothing shows the move taken back', async () => {
		const { room, store } = game('O', undefined, [], { takebacks: true });
		room.receive({ type: 'Moved', ply: 0, cell: top[0] });
		await expect.poll(() => store.saves.length).toBe(1);
		room.receive({ type: 'Offer', kind: 'Takeback', by: 'X' });
		room.answer(true);
		room.receive({ type: 'OfferAnswered', kind: 'Takeback', accepted: true });
		room.receive({ type: 'TookBack', to_ply: 0 });
		expect(room.moves).toEqual([]);
		await expect.poll(() => store.saves.length).toBe(2);
		expect(store.last?.record.moves).toEqual([]);
	});

	it('says when its takeback lapsed, and lets it be asked again', () => {
		const { room } = game('O', undefined, top.slice(0, 2), { takebacks: true });
		room.controls.askTakeback?.run();
		room.receive({ type: 'Offer', kind: 'Takeback', by: 'O' });
		expect(room.controls.askTakeback?.enabled).toBe(false);
		room.receive({ type: 'OfferAnswered', kind: 'Takeback', accepted: false });
		expect(room.notice).toBe('guest-4821 didn’t accept the takeback.');
		expect(room.controls.askTakeback?.enabled).toBe(true);
	});

	it('resyncs on a takeback past what it holds', () => {
		const { room, link } = game('X', undefined, top.slice(0, 2), { takebacks: true });
		room.receive({ type: 'TookBack', to_ply: 3 });
		expect(link.resync).toHaveBeenCalledOnce();
	});

	it('offers a rematch once the game is over, and follows it', () => {
		const { room, sent, onrematch } = game('O', undefined, top);
		expect(room.rematch.enabled).toBe(true);
		room.rematch.run();
		expect(sent).toEqual([{ type: 'OfferRematch' }]);
		expect(room.rematch.enabled).toBe(false);
		room.receive({ type: 'Offer', kind: 'Rematch', by: 'O' });
		room.receive({ type: 'OfferAnswered', kind: 'Rematch', accepted: true });
		room.receive({ type: 'RematchRoom', room: NEXT });
		expect(onrematch).toHaveBeenCalledWith(NEXT);
		expect(room.rematchRoom).toBe(NEXT);
		expect(room.rematch.enabled).toBe(false);
	});

	it('takes the Room’s end, offer and rematch from a Welcome', () => {
		const end = { result: '1-0', termination: 'resign' } as const;
		const { room } = game('X', undefined, top.slice(0, 2), { end, rematch: NEXT });
		expect(room.end).toEqual({ resigned: 'O' });
		expect(room.rematchRoom).toBe(NEXT);
		const offer = { kind: 'Draw', by: 'O' } as const;
		room.welcome(welcome('X', undefined, top.slice(0, 2), { offer }));
		expect(room.end).toBeNull();
		expect(room.offer).toEqual({ kind: 'Draw', mine: false, other: 'guest-1066', lapses: null });
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
