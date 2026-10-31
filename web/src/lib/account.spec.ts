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

	it('gives nothing for an answer that isn’t the server’s', async () => {
		const answers = [
			new Response('not found', { status: 404 }),
			// A static host's fallback page, where the server would be.
			new Response('<!doctype html>', { headers: { 'content-type': 'text/html' } }),
			json({ handle: 'guest-4821' }, 503),
			json({ name: 'guest-4821' }),
			json(null)
		];
		for (const answer of answers) expect(await startSession(async () => answer)).toBeUndefined();
	});

	it('gives nothing offline', async () => {
		const offline = async () => {
			throw new TypeError('Failed to fetch');
		};
		expect(await startSession(offline)).toBeUndefined();
	});
});
