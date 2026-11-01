/** What a page says on a Room's socket, and reading what the server says back (spec §10.3). */
import { protocolVersion, rulesVersion, type ClientMsg, type ServerMsg } from '#lib/engine.js';
import { field, isPlayer, readPlayers } from './api.js';

/** The `Hello` that opens every socket, naming this build. Needs the engine loaded. */
export const hello = (room: string): ClientMsg => ({
	type: 'Hello',
	protocol: protocolVersion(),
	rules_version: rulesVersion(),
	room,
	role: 'Player'
});

/**
 * The message in a frame, if it's one this build knows the shape of. A `Welcome`'s Record is
 * checked when the game opens it, and anything else reads as nothing, so the page reconnects.
 */
export const readServerMsg = (data: unknown): ServerMsg | null => {
	if (typeof data !== 'string') return null;
	let body: unknown;
	try {
		body = JSON.parse(data);
	} catch {
		return null;
	}
	const known = (() => {
		switch (field(body, 'type')) {
			case 'Welcome': {
				const snapshot = field(body, 'snapshot');
				const record = field(snapshot, 'record');
				return (
					isPlayer(field(body, 'you')) &&
					readPlayers(field(snapshot, 'players')) !== undefined &&
					typeof record === 'object' &&
					record !== null
				);
			}
			case 'Moved':
				return count(field(body, 'ply')) && count(field(body, 'cell'));
			case 'Rejected':
				return field(body, 'reason') !== undefined;
			case 'Ended':
				return field(body, 'outcome') !== undefined;
			default:
				return false;
		}
	})();
	return known ? (body as ServerMsg) : null;
};

const count = (n: unknown): boolean => Number.isInteger(n) && (n as number) >= 0;
