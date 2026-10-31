import { describe, expect, it, vi } from 'vitest';
import { startSession } from './account.js';

const json = (body: unknown, status = 200) =>
	new Response(JSON.stringify(body), { status, headers: { 'content-type': 'application/json' } });

describe('starting a session', () => {
	it('posts to the server and gives the Handle it answers with', async () => {
		const fetch = vi.fn(async () => json({ handle: 'guest-4821' }));
		expect(await startSession(fetch)).toEqual({ handle: 'guest-4821' });
		expect(fetch).toHaveBeenCalledWith('/api/session', { method: 'POST' });
	});

	it.each([
		['a missing route', () => new Response('not found', { status: 404 })],
		[
			'a static host’s fallback page',
			() => new Response('<!doctype html>', { headers: { 'content-type': 'text/html' } })
		],
		['JSON not typed as JSON', () => new Response('{"handle":"guest-4821"}')],
		['the database down', () => json({ handle: 'guest-4821' }, 503)],
		['no Handle', () => json({ name: 'guest-4821' })],
		['a Handle that isn’t text', () => json({ handle: 42 })],
		['null', () => json(null)]
	])('gives nothing for %s', async (_, answer) => {
		expect(await startSession(async () => answer())).toBeUndefined();
	});

	it('gives nothing offline', async () => {
		const offline = async () => {
			throw new TypeError('Failed to fetch');
		};
		expect(await startSession(offline)).toBeUndefined();
	});
});
