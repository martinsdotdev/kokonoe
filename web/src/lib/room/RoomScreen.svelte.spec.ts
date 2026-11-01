import { beforeAll, describe, expect, it, vi } from 'vitest';
import { render } from 'vitest-browser-svelte';
import { CloseCode, loadEngine } from '#lib/engine.js';
import { ROOM, sockets, welcome } from './room.fixtures.js';
import RoomScreen from './RoomScreen.svelte';

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
				you: 'X'
			});

describe('a Room’s page', () => {
	it('offers to try again offline', async () => {
		const offline = vi.fn(async () => {
			throw new TypeError('Failed to fetch');
		});
		const screen = render(RoomScreen, { room: ROOM, options: { fetch: offline } });
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
			options: { fetch: seated, open: fake.open, update }
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
