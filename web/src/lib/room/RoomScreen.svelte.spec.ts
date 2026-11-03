import { beforeAll, describe, expect, it, vi } from 'vitest';
import { render } from 'vitest-browser-svelte';
import { CloseCode, loadEngine, type Clocks } from '#lib/engine.js';
import { TOP_ROW, cells } from '#lib/game/games.fixtures.js';
import { FakeStore, ROOM, sockets, welcome } from './room.fixtures.js';
import RoomScreen from './RoomScreen.svelte';
import { SHELF } from '#lib/game/shelf.js';
import { position } from '#lib/game/view.fixtures.js';

beforeAll(() => loadEngine());

const json = (body: unknown) =>
	new Response(JSON.stringify(body), { headers: { 'content-type': 'application/json' } });

/** A server holding a seat for this page, as X. */
const seated = async (url: string | URL | Request) =>
	String(url) === '/api/session'
		? json({ handle: 'guest-4821' })
		: json({
				host: 'guest-4821',
				variant: 'Standard',
				rules: 'std',
				players: { X: 'guest-4821', O: null },
				you: 'X',
				takebacks: false,
				clock: null
			});

const offline = async (): Promise<Response> => {
	throw new TypeError('Failed to fetch');
};

/** The game this page last saved, unfinished or `finished`. */
const kept = async (finished: boolean) => {
	const store = new FakeStore();
	await store.save({
		id: ROOM,
		record: {
			rules_version: 1,
			ruleset: SHELF[0].ruleset,
			seed: '00000000005eed00',
			moves: [3, 30]
		},
		variant: { name: 'Standard', code: 'std' },
		opponent: { kind: 'room', room: ROOM, you: 'X', players: { X: 'guest-4821', O: 'guest-1066' } },
		resigned: null,
		started: 0,
		updated: 0,
		view: position(() => {}),
		toMove: finished ? null : 'X',
		result: finished ? 'X wins on a line' : null
	});
	return store;
};

describe('a Room’s page', () => {
	it('shows offline the game as this page last saw it', async () => {
		const screen = render(RoomScreen, {
			room: ROOM,
			options: { fetch: offline, store: await kept(false) }
		});
		const figure = screen.getByRole('figure', { name: 'Your move against guest-1066' });
		await expect.element(figure).toBeVisible();
		await expect
			.element(screen.getByRole('img', { name: 'The board as you last saw it, after 2 moves' }))
			.toBeVisible();
		await expect
			.element(screen.getByRole('link', { name: 'Replay this game' }))
			.not.toBeInTheDocument();
	});

	it('offers a finished game’s Replay offline', async () => {
		const screen = render(RoomScreen, {
			room: ROOM,
			options: { fetch: offline, store: await kept(true) }
		});
		await expect.element(screen.getByRole('figure', { name: 'X wins on a line' })).toBeVisible();
		await expect
			.element(screen.getByRole('link', { name: 'Replay this game' }))
			.toHaveAttribute('href', `/replay?game=${ROOM}`);
	});

	it('shows no board offline when nothing was kept', async () => {
		const screen = render(RoomScreen, {
			room: ROOM,
			options: { fetch: offline, store: new FakeStore() }
		});
		await expect.element(screen.getByRole('heading', { name: 'You’re offline' })).toBeVisible();
		await expect.element(screen.getByRole('figure')).not.toBeInTheDocument();
	});

	it('offers to try again offline', async () => {
		const offline = vi.fn(async () => {
			throw new TypeError('Failed to fetch');
		});
		const screen = render(RoomScreen, {
			room: ROOM,
			options: { fetch: offline, store: new FakeStore() }
		});
		await expect.element(screen.getByRole('heading', { name: 'You’re offline' })).toBeVisible();
		await screen.getByRole('button', { name: 'Try again' }).click();
		await expect.poll(() => offline.mock.calls.length).toBe(2);
	});

	it('asks for a reload when the newer build can’t be installed', async () => {
		const fake = sockets();
		const update = vi.fn(async () => {
			throw new Error('The newer build didn’t install.');
		});
		const screen = render(RoomScreen, {
			room: ROOM,
			options: { fetch: seated, open: fake.open, update, store: new FakeStore() }
		});
		await expect.poll(() => fake.opened.length).toBe(1);
		fake.last().say(welcome('X', { X: 'guest-4821', O: null }));
		await expect
			.element(screen.getByRole('heading', { name: 'Waiting for your friend' }))
			.toBeVisible();
		fake.last().end(CloseCode.UpdateRequired);
		const heading = screen.getByRole('heading', { name: 'A newer Kokonoe is out' });
		await expect.element(heading).toBeVisible();
		await expect.element(screen.getByRole('button', { name: 'Reload' })).toBeVisible();
		// Focus follows the page to its new view.
		await expect.element(heading).toHaveFocus();
	});

	it('says the game was aborted, and who didn’t make a first move', async () => {
		const fake = sockets();
		const screen = render(RoomScreen, {
			room: ROOM,
			options: { fetch: seated, open: fake.open, store: new FakeStore() }
		});
		await expect.poll(() => fake.opened.length).toBe(1);
		fake.last().say(welcome('X'));
		fake.last().say({ type: 'Aborted', reason: 'FirstMoveTimeout' });
		const heading = screen.getByRole('heading', { name: 'Game aborted' });
		await expect.element(heading).toBeVisible();
		await expect.element(heading).toHaveFocus();
		await expect
			.element(screen.getByText('You didn’t make a first move in time, so the game wasn’t kept.'))
			.toBeVisible();
		await expect.element(screen.getByRole('link', { name: 'Go to Home' })).toBeVisible();
	});
});

describe('a timed Room’s page', () => {
	const top = cells(TOP_ROW);

	/** The page of a timed game this page plays as X, its clocks as the server read them now. */
	const timed = async (moves: number[], clocks: Partial<Clocks>) => {
		const fake = sockets();
		const screen = render(RoomScreen, {
			room: ROOM,
			options: { fetch: seated, open: fake.open, store: new FakeStore() }
		});
		await expect.poll(() => fake.opened.length).toBe(1);
		const now = Date.now();
		const read = { x_ms: 180_000, o_ms: 180_000, running: null, at: now, aborts_at: null };
		fake
			.last()
			.say(welcome('X', undefined, moves, { clock: '3+2', clocks: { ...read, ...clocks } }));
		const spoken = () => document.querySelector('[data-spoken]')?.textContent;
		return { screen, fake, spoken };
	};

	it('shows both clocks, and how long the first move has', async () => {
		const { screen, spoken } = await timed([], { aborts_at: Date.now() + 30_000 });
		await expect
			.element(screen.getByText('Make your first move within 30 seconds, or the game is aborted.'))
			.toBeVisible();
		await expect
			.poll(spoken)
			.toBe('X: play anywhere. You have 30 seconds to make your first move.');
		const clocks = screen.container.querySelectorAll('[data-clock]');
		expect([...clocks].map((clock) => clock.textContent)).toEqual(['3:00', '3:00']);
	});

	it('says this page’s clock is running low as it falls past a cue, and again after a rise', async () => {
		const { fake, spoken } = await timed(top.slice(0, 2), { x_ms: 30_300, running: 'X' });
		await expect.poll(spoken, { timeout: 2_000 }).toBe('30 seconds left.');
		const read = { o_ms: 180_000, aborts_at: null };
		const rose = { ...read, x_ms: 32_000, running: 'O' as const, at: Date.now() };
		fake.last().say({ type: 'Moved', ply: 2, cell: top[2], clocks: rose });
		await expect.poll(spoken).toMatch(/^X played/);
		const fell = { ...read, x_ms: 30_300, running: 'X' as const, at: Date.now() };
		fake.last().say({ type: 'Moved', ply: 3, cell: top[3], clocks: fell });
		await expect.poll(spoken).toMatch(/^O played/);
		await expect.poll(spoken, { timeout: 2_000 }).toBe('30 seconds left.');
	});

	it('leaves a cue passed before the page opened unsaid, and says the next', async () => {
		const { screen, spoken } = await timed(top.slice(0, 2), { x_ms: 10_400, running: 'X' });
		await expect.poll(spoken).toMatch(/^O played/);
		await expect.poll(spoken, { timeout: 2_000 }).toBe('10 seconds left.');
		await expect.element(screen.getByText(/^0:09\.\d$/)).toBeVisible();
		const low = screen.container.querySelector('[data-low]');
		expect(low?.textContent).toMatch(/^0:0\d\.\d$/);
	});

	it('says nothing of the other player’s clock', async () => {
		const { spoken } = await timed(top.slice(0, 3), { o_ms: 30_400, running: 'O' });
		await expect.poll(spoken).toMatch(/^X played/);
		await new Promise((done) => setTimeout(done, 800));
		expect(spoken()).toMatch(/^X played/);
	});
});
