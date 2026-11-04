import { afterEach, beforeAll, beforeEach, describe, expect, it, vi } from 'vitest';
import { loadEngine, type Away, type ClientMsg, type Clocks } from '#lib/engine.js';
import { TOP_ROW, cells } from '#lib/game/games.fixtures.js';
import { UnplayableGame } from '#lib/game/record.js';
import { RoomGame } from './room.svelte.js';
import { FakeStore, ROOM, record, welcome } from './room.fixtures.js';

beforeAll(() => loadEngine());

const top = cells(TOP_ROW);

/**
 * A Room's game whose link the test watches: what it sent, and apart from that the time of each
 * Ping, which every Welcome sends.
 */
const game = (...args: Parameters<typeof welcome>) => {
	const sent: ClientMsg[] = [];
	const pings: number[] = [];
	const link = {
		send: vi.fn((message: ClientMsg) => {
			if (message.type === 'Ping') pings.push(message.t);
			else sent.push(message);
			return true;
		}),
		resync: vi.fn()
	};
	const store = new FakeStore();
	const onrematch = vi.fn();
	const room = new RoomGame(welcome(...args), link, { room: ROOM, store, onrematch });
	return { room, sent, pings, link, store, onrematch };
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
		expect(room.controls?.askTakeback, 'a Room without takebacks').toBeUndefined();
		expect(room.offer).toBeNull();
	});

	it('sends a legal move and waits for the server before showing it', () => {
		const { room, sent } = game('X');
		room.play(top[0]);
		room.play(top[0]);
		expect(sent).toEqual([{ type: 'Move', ply: 0, cell: top[0] }]);
		expect(room.moves).toEqual([]);
		expect(room.frozen).toBe(true);

		room.receive({ type: 'Moved', ply: 0, cell: top[0], clocks: null });
		expect(room.moves).toHaveLength(1);
		expect(room.pending).toBeNull();
		expect(room.replied).toBe(false);
		room.receive({ type: 'Moved', ply: 1, cell: top[1], clocks: null });
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
		room.receive({ type: 'Moved', ply: 1, cell: top[1], clocks: null });
		expect(link.resync).not.toHaveBeenCalled();
		expect(room.moves).toHaveLength(2);
		room.receive({ type: 'Moved', ply: 3, cell: top[3], clocks: null });
		room.receive({ type: 'Moved', ply: 2, cell: top[0], clocks: null });
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
		room.controls?.resign();
		expect(sent).toEqual([{ type: 'Resign' }]);
		room.receive({ type: 'Ended', result: '0-1', termination: 'resign' });
		expect(room.end).toEqual({ resigned: 'X' });
		expect(room.frozen).toBe(true);
		expect(room.controls?.resigner).toBeNull();
		expect(room.file.end).toEqual({ resigned: 'X' });
		await expect.poll(() => store.last?.result).toBe('X resigned. O wins.');
		expect(store.last?.resigned).toBe('X');
	});

	it('offers a draw, shows it at once, and hears when it isn’t accepted', () => {
		const { room, sent } = game('X', undefined, top.slice(0, 2));
		room.controls?.offerDraw?.run();
		expect(sent).toEqual([{ type: 'OfferDraw' }]);
		expect(room.offer).toEqual({ kind: 'Draw', mine: true, other: 'guest-1066', lapses: null });
		expect(room.controls?.offerDraw?.enabled, 'one offer at a time').toBe(false);
		room.receive({ type: 'Offer', kind: 'Draw', by: 'X', lapses_in: null });
		room.receive({ type: 'OfferAnswered', kind: 'Draw', accepted: false });
		expect(room.offer).toBeNull();
		expect(room.notice).toBe('guest-1066 didn’t accept the draw.');
		room.receive({ type: 'Moved', ply: 2, cell: top[2], clocks: null });
		expect(room.notice, 'until the next move').toBeNull();
	});

	it('answers the other player’s draw, and ends agreed', async () => {
		const { room, sent, store } = game('X', undefined, top.slice(0, 2));
		room.receive({ type: 'Offer', kind: 'Draw', by: 'O', lapses_in: null });
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
		room.receive({ type: 'Offer', kind: 'Draw', by: 'X', lapses_in: null });
		room.receive({ type: 'OfferAnswered', kind: 'Draw', accepted: false });
		expect(room.controls?.offerDraw?.enabled).toBe(false);
	});

	it('drops an offer a move withdraws, as the server does', () => {
		const { room } = game('X', undefined, top.slice(0, 2), { takebacks: true });
		room.receive({ type: 'Offer', kind: 'Draw', by: 'X', lapses_in: null });
		room.receive({ type: 'Moved', ply: 2, cell: top[2], clocks: null });
		room.receive({ type: 'OfferAnswered', kind: 'Draw', accepted: false });
		expect(room.offer).toBeNull();
		expect(room.notice, 'its own move withdrew it').toBeNull();

		room.receive({ type: 'Offer', kind: 'Draw', by: 'X', lapses_in: null });
		room.receive({ type: 'Moved', ply: 3, cell: top[3], clocks: null });
		expect(room.offer, 'a draw offer stands through the other’s move').toMatchObject({
			kind: 'Draw'
		});
		room.receive({ type: 'OfferAnswered', kind: 'Draw', accepted: false });

		room.receive({ type: 'Offer', kind: 'Takeback', by: 'O', lapses_in: 15_000 });
		room.receive({ type: 'Moved', ply: 4, cell: top[4], clocks: null });
		expect(room.offer, 'any move withdraws a takeback').toBeNull();
	});

	it('asks a takeback where the Room allows them, and takes the game back', async () => {
		const { room, sent, store } = game('O', undefined, top.slice(0, 2), { takebacks: true });
		expect(room.controls?.askTakeback?.enabled).toBe(true);
		room.controls?.askTakeback?.run();
		expect(sent).toEqual([{ type: 'AskTakeback' }]);
		const heard = Date.now();
		room.receive({ type: 'Offer', kind: 'Takeback', by: 'O', lapses_in: 15_000 });
		expect(room.offer?.lapses).toBeGreaterThanOrEqual(heard + 15_000);
		room.receive({ type: 'OfferAnswered', kind: 'Takeback', accepted: true });
		room.receive({ type: 'TookBack', to_ply: 1, clocks: null });
		expect(room.moves.map((move) => move.notation)).toEqual(['1·4']);
		expect(room.toMove).toBe('O');
		expect(room.offer).toBeNull();
		await expect.poll(() => store.last?.record.moves).toEqual([top[0]]);
	});

	it('counts a waiting takeback down from what a Welcome says it has left', () => {
		const offer = { kind: 'Takeback', by: 'O', lapses_in: 9000 } as const;
		const before = Date.now();
		const { room } = game('X', undefined, top.slice(0, 2), { takebacks: true, offer });
		const after = Date.now();
		expect(room.offer?.lapses).toBeGreaterThanOrEqual(before + 9000);
		expect(room.offer?.lapses).toBeLessThanOrEqual(after + 9000);
	});

	it('needs a move of this player’s own to ask a takeback', () => {
		const { room } = game('O', undefined, top.slice(0, 1), { takebacks: true });
		expect(room.controls?.askTakeback?.enabled).toBe(false);
	});

	it('keeps a takeback to before every move, so nothing shows the move taken back', async () => {
		const { room, store } = game('O', undefined, [], { takebacks: true });
		room.receive({ type: 'Moved', ply: 0, cell: top[0], clocks: null });
		await expect.poll(() => store.saves.length).toBe(1);
		room.receive({ type: 'Offer', kind: 'Takeback', by: 'X', lapses_in: 15_000 });
		room.answer(true);
		room.receive({ type: 'OfferAnswered', kind: 'Takeback', accepted: true });
		room.receive({ type: 'TookBack', to_ply: 0, clocks: null });
		expect(room.moves).toEqual([]);
		await expect.poll(() => store.saves.length).toBe(2);
		expect(store.last?.record.moves).toEqual([]);
	});

	it('says when its takeback lapsed, and lets it be asked again', () => {
		const { room } = game('O', undefined, top.slice(0, 2), { takebacks: true });
		room.controls?.askTakeback?.run();
		room.receive({ type: 'Offer', kind: 'Takeback', by: 'O', lapses_in: 15_000 });
		expect(room.controls?.askTakeback?.enabled).toBe(false);
		room.receive({ type: 'OfferAnswered', kind: 'Takeback', accepted: false });
		expect(room.notice).toBe('guest-4821 didn’t accept the takeback.');
		expect(room.controls?.askTakeback?.enabled).toBe(true);
	});

	it('resyncs on a takeback past what it holds', () => {
		const { room, link } = game('X', undefined, top.slice(0, 2), { takebacks: true });
		room.receive({ type: 'TookBack', to_ply: 3, clocks: null });
		expect(link.resync).toHaveBeenCalledOnce();
	});

	it('offers a rematch once the game is over, and follows it', () => {
		const { room, sent, onrematch } = game('O', undefined, top);
		expect(room.rematch?.enabled).toBe(true);
		room.rematch?.run();
		expect(sent).toEqual([{ type: 'OfferRematch' }]);
		expect(room.rematch?.enabled).toBe(false);
		room.receive({ type: 'Offer', kind: 'Rematch', by: 'O', lapses_in: 60_000 });
		room.receive({ type: 'OfferAnswered', kind: 'Rematch', accepted: true });
		room.receive({ type: 'RematchRoom', room: NEXT });
		expect(onrematch).toHaveBeenCalledWith(NEXT);
		expect(room.rematchRoom).toBe(NEXT);
		expect(room.rematch?.enabled).toBe(false);
	});

	it('takes the Room’s end, offer and rematch from a Welcome', () => {
		const end = { result: '1-0', termination: 'resign' } as const;
		const { room } = game('X', undefined, top.slice(0, 2), { end, rematch: NEXT });
		expect(room.end).toEqual({ resigned: 'O' });
		expect(room.rematchRoom).toBe(NEXT);
		const offer = { kind: 'Draw', by: 'O', lapses_in: null } as const;
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
		room.receive({ type: 'Moved', ply: 0, cell: top[0], clocks: null });
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
		room.receive({ type: 'Moved', ply: 0, cell: top[0], clocks: null });
		room.receive({ type: 'Moved', ply: 1, cell: top[1], clocks: null });
		await expect
			.poll(() => store.saves.map((saved) => saved.record.moves))
			.toEqual([[top[0]], [top[0], top[1]]]);
		room.receive({ type: 'Moved', ply: 0, cell: top[0], clocks: null });
		room.receive({ type: 'Moved', ply: 5, cell: top[5], clocks: null });
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

describe('a timed Room’s game', () => {
	/** This page's time when each test begins. */
	const T = 1_793_600_000_000;

	beforeEach(() => {
		vi.useFakeTimers();
		vi.setSystemTime(T);
	});

	afterEach(() => {
		vi.useRealTimers();
	});

	/** The clocks as a server 5 s ahead of this page read them as each test begins. */
	const clocks = (edit: Partial<Clocks> = {}): Clocks => ({
		x_ms: 180_000,
		o_ms: 180_000,
		running: null,
		at: T + 5_000,
		aborts_at: T + 35_000,
		...edit
	});

	const timed = (you: 'X' | 'O' = 'X', moves: number[] = [], edit: Partial<Clocks> = {}) =>
		game(you, undefined, moves, { clock: '3+2', clocks: clocks(edit) });

	it('shows a Welcome’s clocks in this page’s time before anything measures it', () => {
		const { room } = timed();
		expect(room.clock).toBe('3+2');
		expect(room.clocks).toEqual({
			ms: { X: 180_000, O: 180_000 },
			running: null,
			at: T,
			aborts: T + 30_000
		});
	});

	it('measures the server’s time with three Pings after each Welcome, keeping the quickest', () => {
		const { room, pings } = timed();
		const pong = (wait: number, server: number) => {
			const t = pings.at(-1) ?? 0;
			vi.advanceTimersByTime(wait);
			room.receive({ type: 'Pong', t, server_ms: server });
		};
		pong(100, T + 5_030);
		expect(room.clocks?.at, 'over 100 ms, 4,980 ms ahead').toBe(T + 20);
		pong(40, T + 5_110);
		expect(room.clocks?.at, 'over 40 ms, 4,990 ms ahead').toBe(T + 10);
		pong(300, T + 5_500);
		expect(room.clocks?.at, 'over 300 ms, which is kept for nothing').toBe(T + 10);
		room.receive({ type: 'Pong', t: Date.now() + 1_000, server_ms: T });
		expect(room.clocks?.at, 'over a round trip this page’s clock ran back through').toBe(T + 10);
		expect(pings).toEqual([T, T + 100, T + 140]);
		room.welcome(welcome('X', undefined, [], { clock: '3+2', clocks: clocks() }));
		expect(pings.at(-1), 'a Welcome measures it again').toBe(T + 440);
		expect(room.clocks?.at, 'keeping what was measured').toBe(T + 10);
	});

	it('takes the clocks from each move and takeback, and none from a move it has', () => {
		const { room } = timed();
		room.receive({
			type: 'Moved',
			ply: 0,
			cell: top[0],
			clocks: clocks({ aborts_at: T + 65_000 })
		});
		expect(room.clocks?.aborts).toBe(T + 60_000);
		const running = clocks({ running: 'X', aborts_at: null });
		room.receive({ type: 'Moved', ply: 1, cell: top[1], clocks: running });
		expect(room.clocks?.running).toBe('X');
		room.receive({ type: 'Moved', ply: 1, cell: top[1], clocks: clocks({ x_ms: 1 }) });
		expect(room.clocks?.ms.X, 'a resend').toBe(180_000);
		room.receive({ type: 'TookBack', to_ply: 1, clocks: clocks({ aborts_at: T + 90_000 }) });
		expect(room.clocks).toMatchObject({ running: null, aborts: T + 85_000 });
	});

	it('ends on time without resyncing, the clock that ran out at 0', async () => {
		const { room, link, store } = timed('X', top.slice(0, 2), { running: 'X', aborts_at: null });
		room.receive({ type: 'Ended', result: '0-1', termination: 'timeout' });
		expect(room.end).toEqual({ flagged: 'X' });
		expect(link.resync).not.toHaveBeenCalled();
		expect(room.clocks).toMatchObject({ ms: { X: 0, O: 180_000 }, running: null, aborts: null });
		await expect.poll(() => store.last?.result).toBe('X ran out of time. O wins.');
		expect(store.last).toMatchObject({ flagged: 'X', clock: '3+2' });
		expect(room.file).toMatchObject({ end: { flagged: 'X' }, clock: '3+2' });
	});

	it('stops the clocks where they stand when the game ends', () => {
		const { room } = timed('X', top.slice(0, 2), { running: 'X', aborts_at: null, x_ms: 50_000 });
		vi.advanceTimersByTime(1_500);
		room.receive({ type: 'Ended', result: '0-1', termination: 'resign' });
		expect(room.clocks).toMatchObject({ ms: { X: 48_500, O: 180_000 }, running: null });
	});
});

describe('a player away from the Room', () => {
	/** This page's time when each test begins. */
	const T = 1_793_600_000_000;

	beforeEach(() => {
		vi.useFakeTimers();
		vi.setSystemTime(T);
	});

	afterEach(() => {
		vi.useRealTimers();
	});

	const players = { X: 'guest-4821', O: 'guest-1066' };

	/** Away `ago` ms before this page's now, by a server 5 s ahead, with `claimsIn` left to wait. */
	const gone = (ago: number, claimsIn = 60_000): Away => ({
		since: T + 5_000 - ago,
		claims_in: claimsIn
	});

	/** A game this page plays as X, with O away as the Welcome says. */
	const left = (away: Away) =>
		game('X', players, top.slice(0, 2), { presence: { X: null, O: away } });

	it('measures the server’s time in an untimed Room too, for how long a player has been away', () => {
		const { room, pings } = left(gone(10_000));
		expect(pings).toEqual([T]);
		expect(room.clocks).toBeNull();
		expect(room.away, 'the server’s time taken for the page’s').toEqual({ X: null, O: T - 5_000 });
		vi.advanceTimersByTime(100);
		room.receive({ type: 'Pong', t: T, server_ms: T + 5_050 });
		expect(room.away.O, 'measured: 10 s before the Welcome').toBe(T - 10_000);
	});

	it('hears the other player leave and come back', () => {
		const { room } = game('X', players);
		room.receive({ type: 'Presence', player: 'O', away: gone(0) });
		expect(room.away.O).not.toBeNull();
		expect(room.notice).toBe('guest-1066 left the game.');
		expect(room.claim).toMatchObject({ other: 'guest-1066', due: T + 60_000, enabled: true });
		room.receive({ type: 'Presence', player: 'O', away: null });
		expect(room.away.O).toBeNull();
		expect(room.claim).toBeNull();
		expect(room.notice).toBe('guest-1066 is back.');
	});

	it('says no one is back that it didn’t see leave, and nothing of its own seat', () => {
		const { room } = game('X', players, [], { presence: { X: gone(0), O: null } });
		expect(room.away.X, 'this page’s own seat').toBeNull();
		room.receive({ type: 'Presence', player: 'O', away: null });
		room.receive({ type: 'Presence', player: 'X', away: gone(0) });
		expect(room.notice).toBeNull();
		expect(room.away).toEqual({ X: null, O: null });
		expect(room.claim).toBeNull();
	});

	it('claims the win once it’s due, only once, and ends without resyncing', async () => {
		const { room, sent, link, store } = left(gone(0, 1_000));
		room.claim?.run('Win');
		expect(sent, 'not yet due').toEqual([]);
		vi.advanceTimersByTime(1_000);
		room.claim?.run('Win');
		room.claim?.run('Win');
		expect(sent).toEqual([{ type: 'Claim', as: 'Win' }]);
		expect(room.claim?.enabled, 'waiting for the server').toBe(false);
		room.receive({ type: 'Ended', result: '1-0', termination: 'claim' });
		expect(room.end).toEqual({ claimed: 'X' });
		expect(link.resync).not.toHaveBeenCalled();
		expect(room.claim, 'once the game is over').toBeNull();
		expect(room.away.O, 'no one is kept waiting once it’s over').toBeNull();
		room.receive({ type: 'Presence', player: 'O', away: null });
		room.receive({ type: 'Presence', player: 'O', away: gone(0) });
		expect(room.notice, 'nothing said of it after the end').toBeNull();
		await expect.poll(() => store.last?.result).toBe('O left the game. X claimed the win.');
		expect(store.last).toMatchObject({ claimed: 'X' });
		expect(room.file.end).toEqual({ claimed: 'X' });
	});

	it('claims a draw', async () => {
		const { room, sent, store } = game('O', players, top.slice(0, 2), {
			presence: { X: gone(65_000, 0), O: null }
		});
		room.claim?.run('Draw');
		expect(sent).toEqual([{ type: 'Claim', as: 'Draw' }]);
		room.receive({ type: 'Ended', result: '1/2-1/2', termination: 'claim' });
		expect(room.end).toEqual({ claimedDraw: true });
		await expect.poll(() => store.last?.result).toBe('Draw claimed');
		expect(store.last).toMatchObject({ claimedDraw: true });
	});

	it('offers no Claim before both seats are filled, after the game, or while the connection is down', () => {
		const presence = { X: null, O: gone(0, 0) };
		const waiting = game('X', { X: 'guest-4821', O: null }, [], { presence });
		expect(waiting.room.claim).toBeNull();
		const end = { result: '1-0', termination: 'resign' } as const;
		const over = game('X', players, top.slice(0, 2), { presence, end });
		expect(over.room.claim).toBeNull();
		const { room, sent } = left(gone(0, 0));
		room.dropped();
		expect(room.claim?.enabled).toBe(false);
		room.claim?.run('Win');
		expect(sent).toEqual([]);
	});

	it('resyncs when the server refuses a Claim, and may claim again after', () => {
		const { room, link } = left(gone(0, 0));
		room.claim?.run('Win');
		room.receive({ type: 'Rejected', reason: 'NotAllowed' });
		expect(link.resync).toHaveBeenCalled();
		const presence = { X: null, O: gone(0, 0) };
		room.welcome(welcome('X', players, top.slice(0, 2), { presence }));
		expect(room.claim?.enabled).toBe(true);
	});

	it('ends a game both players left as abandoned, as a Welcome says', async () => {
		const { room, store } = game('X', players, top.slice(0, 2));
		await expect.poll(() => store.last?.result).toBeNull();
		const end = { result: null, termination: 'abandoned' } as const;
		room.welcome(welcome('X', players, top.slice(0, 2), { end }));
		expect(room.end).toEqual({ abandoned: true });
		await expect.poll(() => store.last?.result).toBe('Game abandoned');
		expect(store.last).toMatchObject({ abandoned: true, toMove: null });
		expect(room.file.end).toEqual({ abandoned: true });
		expect(room.rematch?.enabled, 'both may still play again').toBe(true);
	});
});
