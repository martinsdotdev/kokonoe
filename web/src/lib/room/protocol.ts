/** What a page says on a Room's socket, and reading what the server says back (spec §10.3). */
import {
	protocolVersion,
	rulesVersion,
	type ClientMsg,
	type GameResult,
	type OfferKind,
	type ServerMsg,
	type Termination
} from '#lib/engine.js';
import { isClock } from '#lib/game/rules.js';
import { field, isPlayer, isRoomId, readPlayers } from './api.js';

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
				const offer = field(snapshot, 'offer');
				const end = field(snapshot, 'end');
				const rematch = field(snapshot, 'rematch');
				const draws = field(snapshot, 'draws');
				return (
					isPlayer(field(body, 'you')) &&
					readPlayers(field(snapshot, 'players')) !== undefined &&
					typeof record === 'object' &&
					record !== null &&
					typeof field(snapshot, 'takebacks') === 'boolean' &&
					(offer === null || isOffer(offer)) &&
					count(field(draws, 'X')) &&
					count(field(draws, 'O')) &&
					(end === null || isEnd(end)) &&
					(rematch === null || isRoomId(rematch)) &&
					nullOr(isClock, field(snapshot, 'clock')) &&
					nullOr(isClocks, field(snapshot, 'clocks'))
				);
			}
			case 'Moved':
				return (
					count(field(body, 'ply')) &&
					count(field(body, 'cell')) &&
					nullOr(isClocks, field(body, 'clocks'))
				);
			case 'Rejected':
				return field(body, 'reason') !== undefined;
			case 'Ended':
				return isEnd(body);
			case 'Offer':
				return isOffer(body);
			case 'OfferAnswered':
				return isKind(field(body, 'kind')) && typeof field(body, 'accepted') === 'boolean';
			case 'TookBack':
				return count(field(body, 'to_ply')) && nullOr(isClocks, field(body, 'clocks'));
			case 'RematchRoom':
				return isRoomId(field(body, 'room'));
			case 'Aborted':
				return field(body, 'reason') === 'FirstMoveTimeout';
			case 'Pong':
				return count(field(body, 't')) && count(field(body, 'server_ms'));
			default:
				return false;
		}
	})();
	return known ? (body as ServerMsg) : null;
};

const count = (n: unknown): boolean => Number.isInteger(n) && (n as number) >= 0;

/** Whether `value` is one of `table`'s keys, which list every member of a union. */
const among = (table: object, value: unknown): boolean =>
	typeof value === 'string' && Object.hasOwn(table, value);

const KINDS: Record<OfferKind, true> = { Draw: true, Takeback: true, Rematch: true };
const RESULTS: Record<GameResult, true> = { '1-0': true, '0-1': true, '1/2-1/2': true };
const TERMINATIONS: Record<Termination, true> = {
	line: true,
	count: true,
	decided: true,
	resign: true,
	'agreed draw': true,
	timeout: true
};

const isKind = (kind: unknown): boolean => among(KINDS, kind);

const isOffer = (offer: unknown): boolean => {
	const lapses = field(offer, 'lapses_in');
	return (
		isKind(field(offer, 'kind')) &&
		isPlayer(field(offer, 'by')) &&
		(lapses === null || count(lapses))
	);
};

const isEnd = (end: unknown): boolean =>
	among(RESULTS, field(end, 'result')) && among(TERMINATIONS, field(end, 'termination'));

const nullOr = (is: (value: unknown) => boolean, value: unknown): boolean =>
	value === null || is(value);

/** A timed Room's clocks: each player's ms left, whose runs, and the server's times. */
const isClocks = (clocks: unknown): boolean =>
	count(field(clocks, 'x_ms')) &&
	count(field(clocks, 'o_ms')) &&
	nullOr(isPlayer, field(clocks, 'running')) &&
	count(field(clocks, 'at')) &&
	nullOr(count, field(clocks, 'aborts_at'));
