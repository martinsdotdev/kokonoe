import { describe, expect, it, vi } from 'vitest';
import { createRoom, roomCard } from './api.js';

const ROOM = '6f1c0a9b3e2d4c5a7b8e';

const json = (body: unknown, status = 200) =>
	new Response(JSON.stringify(body), { status, headers: { 'content-type': 'application/json' } });

const page = () => new Response('<!doctype html>', { headers: { 'content-type': 'text/html' } });

const offline = async (): Promise<Response> => {
	throw new TypeError('Failed to fetch');
};

const CARD = {
	host: 'guest-4821',
	variant: 'Standard',
	rules: 'std',
	players: { X: 'guest-4821', O: null },
	you: null,
	takebacks: false,
	clock: null,
	spectators: false
};

describe('making a Room', () => {
	it('posts the host’s choice and gives the Room’s id', async () => {
		const fetch = vi.fn(async () => json({ room: ROOM }, 201));
		expect(
			await createRoom(
				{ rules: 'std', seat: 'Random', takebacks: false, clock: null, spectators: false },
				fetch
			)
		).toBe(ROOM);
		expect(fetch).toHaveBeenCalledWith('/api/rooms', {
			method: 'POST',
			headers: { 'content-type': 'application/json' },
			body: '{"rules":"std","seat":"Random","takebacks":false,"clock":null,"spectators":false}'
		});
	});

	it.each([
		['offline', offline],
		['without a session', async () => new Response('no session', { status: 403 })],
		['a static host’s fallback page', async () => page()],
		['an id of another shape', async () => json({ room: '../oops' }, 201)]
	])('gives nothing %s', async (_, fetch) => {
		expect(
			await createRoom(
				{ rules: 'std', seat: 'X', takebacks: false, clock: null, spectators: false },
				fetch
			)
		).toBeUndefined();
	});
});

describe('reading a Room’s card', () => {
	it('gives the card the server answers', async () => {
		const fetch = vi.fn(async () => json(CARD));
		expect(await roomCard(ROOM, fetch)).toEqual(CARD);
		expect(fetch).toHaveBeenCalledWith(`/api/rooms/${ROOM}`);
		const watched = { ...CARD, spectators: true };
		expect(await roomCard(ROOM, async () => json(watched))).toEqual(watched);
	});

	it('knows a Room is missing from its id alone, or from the server', async () => {
		const fetch = vi.fn(offline);
		expect(await roomCard('abc', fetch)).toBe('missing');
		expect(fetch).not.toHaveBeenCalled();
		const missing = async () => new Response('no such Room', { status: 404 });
		expect(await roomCard(ROOM, missing)).toBe('missing');
	});

	it.each([
		['offline', offline],
		['a static host’s fallback page', async () => page()],
		[
			'a static host’s missing page',
			async () =>
				new Response('<p>404</p>', { status: 404, headers: { 'content-type': 'text/html' } })
		],
		['the database down', async () => json(CARD, 503)],
		['a card without its players', async () => json({ ...CARD, players: null })],
		['a seat that isn’t a Player', async () => json({ ...CARD, you: 'Z' })],
		[
			'a card that doesn’t say if it can be watched',
			async () => json({ ...CARD, spectators: undefined })
		]
	])('gives nothing for %s', async (_, fetch) => {
		expect(await roomCard(ROOM, fetch)).toBeUndefined();
	});
});
