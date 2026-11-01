/**
 * The Room routes (spec §10.1): making an invite Room, and reading the card its link shows a
 * joiner. Engine-free, so Home and the New game sheet can make one.
 */
import type { NewRoom, Player, Players, RoomCard } from '#lib/engine.js';

/** A Room's id: 20 lowercase hex digits, as the server writes them. */
export const isRoomId = (text: string): boolean => /^[0-9a-f]{20}$/.test(text);

/**
 * Makes a Room for the session's Account and gives its id, or nothing when the server can't make
 * one: offline, without a session, or from a build served without it.
 */
export async function createRoom(
	asked: NewRoom,
	fetch = globalThis.fetch
): Promise<string | undefined> {
	const body = await json(
		fetch('/api/rooms', {
			method: 'POST',
			headers: { 'content-type': 'application/json' },
			body: JSON.stringify(asked)
		})
	);
	const room = field(body, 'room');
	return typeof room === 'string' && isRoomId(room) ? room : undefined;
}

/**
 * What the Room `room`'s link asks its holder to join: `missing` when there's no such Room, or
 * nothing when the server can't be reached.
 */
export async function roomCard(
	room: string,
	fetch = globalThis.fetch
): Promise<RoomCard | 'missing' | undefined> {
	if (!isRoomId(room)) return 'missing';
	let response: Response;
	try {
		response = await fetch(`/api/rooms/${room}`);
	} catch {
		return undefined;
	}
	if (response.status === 404 && !html(response)) return 'missing';
	return readCard(await json(Promise.resolve(response)));
}

/** The card in `body`, if that's what it holds. */
export const readCard = (body: unknown): RoomCard | undefined => {
	const [host, variant, rules] = ['host', 'variant', 'rules'].map((key) => field(body, key));
	const players = readPlayers(field(body, 'players'));
	const you = field(body, 'you');
	const strings = [host, variant, rules].every((value) => typeof value === 'string');
	if (!strings || !players || !(you === null || isPlayer(you))) return undefined;
	return { host, variant, rules, players, you } as RoomCard;
};

export const readPlayers = (body: unknown): Players | undefined => {
	const [x, o] = [field(body, 'X'), field(body, 'O')];
	const seat = (value: unknown) => value === null || typeof value === 'string';
	return seat(x) && seat(o) ? ({ X: x, O: o } as Players) : undefined;
};

export const isPlayer = (value: unknown): value is Player => value === 'X' || value === 'O';

/** Both seats are taken. */
export const full = (players: Players): boolean => players.X !== null && players.O !== null;

/** `body[key]`, for a body that's an object. */
export const field = (body: unknown, key: string): unknown =>
	typeof body === 'object' && body !== null && key in body
		? (body as Record<string, unknown>)[key]
		: undefined;

/** An answer's JSON, or nothing for one that isn't the server's: offline, an error, a page. */
const json = async (answer: Promise<Response>): Promise<unknown> => {
	try {
		const response = await answer;
		if (!response.ok || html(response)) return undefined;
		return await response.json();
	} catch {
		return undefined;
	}
};

/** A static host's fallback page, as sirv or Vite send for any path. */
const html = (response: Response) =>
	response.headers.get('content-type')?.startsWith('text/html') === true;
