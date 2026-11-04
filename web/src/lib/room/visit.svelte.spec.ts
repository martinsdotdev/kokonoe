import { beforeAll, describe, expect, it, vi } from 'vitest';
import { CloseCode, loadEngine, type RoomCard } from '#lib/engine.js';
import { FakeStore, ROOM, sockets, welcome } from './room.fixtures.js';
import { RoomVisit } from './visit.svelte.js';

beforeAll(() => loadEngine());

const json = (body: unknown, status = 200) =>
	new Response(JSON.stringify(body), { status, headers: { 'content-type': 'application/json' } });

const card = (players: RoomCard['players'], you: RoomCard['you'] = null): RoomCard => ({
	host: 'guest-4821',
	variant: 'Standard',
	rules: 'std',
	players,
	you,
	takebacks: false,
	clock: null,
	spectators: false
});

/** A server answering the session with a Guest and the card with `cards`, one per ask. */
const server = (...cards: (RoomCard | number)[]) =>
	vi.fn(async (url: string | URL | Request) => {
		if (String(url) === '/api/session') return json({ handle: 'guest-1066' });
		const next = cards.length > 1 ? cards.shift()! : cards[0];
		return typeof next === 'number' ? new Response('', { status: next }) : json(next);
	});

const visit = (
	fetch: typeof globalThis.fetch,
	update = vi.fn(async () => {}),
	store = new FakeStore()
) => {
	const fake = sockets();
	const window = new EventTarget() as Window;
	const made = new RoomVisit(ROOM, { fetch, open: fake.open, update, window, store });
	return { visit: made, fake, window, update, store };
};

const phase = (made: RoomVisit) => expect.poll(() => made.phase.kind);

describe('following a Room’s link', () => {
	it('offers a free seat, and plays once accepted', async () => {
		const { visit: made, fake } = visit(server(card({ X: 'guest-4821', O: null })));
		await phase(made).toBe('join');
		expect(fake.opened).toEqual([]);

		made.accept();
		await expect.poll(() => fake.opened.length).toBe(1);
		fake.last().accept();
		expect(fake.last().sent).toEqual([
			{ type: 'Hello', protocol: 4, rules_version: 1, room: ROOM, role: 'Player' }
		]);
		fake.last().say(welcome('O'));
		await phase(made).toBe('playing');
	});

	it('goes straight in for a seat already held', async () => {
		const { visit: made, fake } = visit(server(card({ X: 'guest-4821', O: null }, 'X')));
		await expect.poll(() => fake.opened.length).toBe(1);
		fake.last().say(welcome('X', { X: 'guest-4821', O: null }));
		await phase(made).toBe('playing');
		if (made.phase.kind !== 'playing') return;
		expect(made.phase.game.seated).toBe(false);
	});

	it('says so when the game is full or missing', async () => {
		const full = visit(server(card({ X: 'guest-4821', O: 'guest-0007' })));
		await phase(full.visit).toBe('full');
		const missing = visit(server(404));
		await phase(missing.visit).toBe('missing');
	});

	it('waits offline, and tries again when back online', async () => {
		const offline = vi.fn<typeof globalThis.fetch>(async () => {
			throw new TypeError('Failed to fetch');
		});
		const { visit: made, window } = visit(offline);
		await phase(made).toBe('offline');
		offline.mockImplementation(server(card({ X: 'guest-4821', O: null })));
		window.dispatchEvent(new Event('online'));
		await phase(made).toBe('join');
	});

	it('shows the game as it was last saved while offline, and keeps when it began', async () => {
		const store = new FakeStore();
		const offline = vi.fn<typeof globalThis.fetch>(async () => {
			throw new TypeError('Failed to fetch');
		});
		const first = visit(server(card({ X: 'a', O: 'b' }, 'X')), undefined, store);
		await expect.poll(() => first.fake.opened.length).toBe(1);
		first.fake.last().say(welcome('X', { X: 'a', O: 'b' }, [3]));
		await expect.poll(() => store.saves.length).toBe(1);
		first.visit.dispose();

		const later = visit(offline, undefined, store);
		await phase(later.visit).toBe('offline');
		if (later.visit.phase.kind !== 'offline') return;
		expect(later.visit.phase.saved?.record.moves).toEqual([3]);

		const back = visit(server(card({ X: 'a', O: 'b' }, 'X')), undefined, store);
		await expect.poll(() => back.fake.opened.length).toBe(1);
		back.fake.last().say(welcome('X', { X: 'a', O: 'b' }, [3, 30]));
		await expect.poll(() => store.saves.length).toBe(2);
		expect(store.last?.started).toBe(store.saves[0].started);
	});

	it('marks the game reconnecting while the socket is down', async () => {
		const { visit: made, fake } = visit(server(card({ X: 'a', O: 'b' }, 'X')));
		await expect.poll(() => fake.opened.length).toBe(1);
		fake.last().say(welcome('X'));
		await phase(made).toBe('playing');
		if (made.phase.kind !== 'playing') return;
		const { game } = made.phase;
		fake.last().end(1006);
		expect(game.connected).toBe(false);
		await expect.poll(() => fake.opened.length).toBe(2);
		fake.last().say(welcome('X'));
		expect(game.connected).toBe(true);
	});

	it('follows the server’s closes', async () => {
		const gone = visit(server(card({ X: 'a', O: null }, 'X')));
		await expect.poll(() => gone.fake.opened.length).toBe(1);
		gone.fake.last().end(CloseCode.NoRoom);
		await phase(gone.visit).toBe('missing');

		const taken = visit(server(card({ X: 'a', O: null }), card({ X: 'a', O: 'b' })));
		await phase(taken.visit).toBe('join');
		taken.visit.accept();
		await expect.poll(() => taken.fake.opened.length).toBe(1);
		taken.fake.last().end(CloseCode.Forbidden);
		await phase(taken.visit).toBe('full');

		const seated = visit(server(card({ X: 'a', O: 'b' }, 'X')));
		await expect.poll(() => seated.fake.opened.length).toBe(1);
		seated.fake.last().end(CloseCode.Forbidden);
		await phase(seated.visit).toBe('problem');

		const older = visit(server(card({ X: 'a', O: null }, 'X')));
		await expect.poll(() => older.fake.opened.length).toBe(1);
		older.fake.last().end(CloseCode.UpdateRequired);
		await expect.poll(() => older.update.mock.calls.length).toBe(1);

		const stuck = visit(
			server(card({ X: 'a', O: null }, 'X')),
			vi.fn(async () => {
				throw new Error('The newer build didn’t install.');
			})
		);
		await expect.poll(() => stuck.fake.opened.length).toBe(1);
		stuck.fake.last().end(CloseCode.UpdateRequired);
		await phase(stuck.visit).toBe('update');
	});
});

describe('an aborted game', () => {
	/** A visit to a game this page plays as `you`, its Welcome said, with `moves` made. */
	const playing = async (you: 'X' | 'O', moves: number[], store = new FakeStore()) => {
		const made = visit(server(card({ X: 'guest-4821', O: 'guest-1066' }, you)), undefined, store);
		await expect.poll(() => made.fake.opened.length).toBe(1);
		made.fake.last().accept();
		made.fake.last().say(welcome(you, undefined, moves));
		await phase(made.visit).toBe('playing');
		return made;
	};

	it('says who didn’t make a first move, closes the socket and deletes this page’s copy', async () => {
		const store = new FakeStore();
		const { visit: made, fake } = await playing('O', [3], store);
		await expect.poll(() => store.saves.length).toBe(1);
		const socket = fake.last();
		socket.say({ type: 'Aborted', reason: 'FirstMoveTimeout' });
		expect(made.phase).toEqual({ kind: 'aborted', late: 'You' });
		expect(socket.closed).toBe(true);
		expect(store.deleted).toEqual([ROOM]);
		socket.end(1011);
		expect(fake.opened, 'the server’s close opens no socket').toHaveLength(1);

		const theirs = await playing('X', [3]);
		theirs.fake.last().say({ type: 'Aborted', reason: 'FirstMoveTimeout' });
		expect(theirs.visit.phase).toEqual({ kind: 'aborted', late: 'guest-1066' });
	});

	it('reads a game gone while this page played it as aborted, unseen', async () => {
		const store = new FakeStore();
		const { visit: made, fake } = await playing('X', [], store);
		fake.last().end(CloseCode.NoRoom);
		await expect.poll(() => made.phase).toEqual({ kind: 'aborted', late: null });
		expect(store.deleted).toEqual([ROOM]);
	});

	it('reads a missing Room this page left unfinished as aborted, and one it saw end as missing', async () => {
		const store = new FakeStore();
		const first = await playing('X', [3], store);
		await expect.poll(() => store.saves.length).toBe(1);
		const copy = store.last;
		first.visit.dispose();
		const later = visit(server(404), undefined, store);
		await expect.poll(() => later.visit.phase).toEqual({ kind: 'aborted', late: null });
		expect(store.deleted).toEqual([ROOM]);

		const ended = new FakeStore();
		await ended.save({ ...copy, toMove: null, result: 'X wins on a line' });
		const gone = visit(server(404), undefined, ended);
		await phase(gone.visit).toBe('missing');
		expect(ended.deleted).toEqual([]);
	});
});
