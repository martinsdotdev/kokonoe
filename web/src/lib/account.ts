/** Who's playing, as the server knows them: for now, always a Guest Account. */
export type Me = { handle: string };

/**
 * Starts a session with the server, or resumes the one its cookie names, which makes a Guest
 * Account on a first visit (spec §9). Offline, from a build served without the server (sirv,
 * Vite), or for any answer that isn't the server's, it gives nothing: every page works without an
 * Account.
 */
export async function startSession(fetch = globalThis.fetch): Promise<Me | undefined> {
	try {
		const response = await fetch('/api/session', { method: 'POST' });
		const json = response.headers.get('content-type')?.startsWith('application/json');
		if (!response.ok || !json) return undefined;
		const me: unknown = await response.json();
		const handle = typeof me === 'object' && me !== null && 'handle' in me ? me.handle : undefined;
		return typeof handle === 'string' ? { handle } : undefined;
	} catch {
		return undefined;
	}
}
