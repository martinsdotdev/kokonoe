import { beforeAll, describe, expect, it, vi } from 'vitest';
import { render } from 'vitest-browser-svelte';
import { CloseCode, loadEngine } from '#lib/engine.js';
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
});
