/**
 * A Room's server, faked for the e2e tests: the session, the Room's routes, and its socket, which
 * answers each `Hello` with a `Welcome` and each `Ping` with a `Pong`, and keeps everything else
 * the page sends. Every Room id gets the same card and `Welcome`, so a test changes the `FakeRoom`
 * before the page follows a rematch.
 */
import type { Page, WebSocketRoute } from '@playwright/test';
import type {
	Away,
	ClientMsg,
	Clocks,
	NewRoom,
	Players,
	RoomCard,
	ServerMsg,
	Snapshot
} from '../lib/engine.js';
import type { SavedGame } from '../lib/game/saved.js';
import { SHELF } from '../lib/game/shelf.js';

export const ROOM = '6f1c0a9b3e2d4c5a7b8e';
/** The Room a rematch makes. */
export const NEXT = 'b'.repeat(20);

export const HOST = 'guest-4821';
export const JOINER = 'guest-1066';

/** A timed Room's clocks as the server reads them now: both at 3+2's bank, until edited. */
export const clocksNow = (edit: Partial<Clocks> = {}): Clocks => ({
	x_ms: 180_000,
	o_ms: 180_000,
	running: null,
	at: Date.now(),
	aborts_at: null,
	...edit
});

/** A player away `ago` ms before now, as the server says it, `claimsIn` ms before a Claim. */
export const awayNow = (ago = 0, claimsIn = 60_000): Away => ({
	since: Date.now() - ago,
	claims_in: claimsIn
});

/** The game this page keeps as `id`, read from IndexedDB itself. */
export const keptGame = (page: Page, id: string): Promise<SavedGame | undefined> =>
	page.evaluate(
		(id) =>
			new Promise<SavedGame | undefined>((resolve, reject) => {
				const open = indexedDB.open('kokonoe');
				open.onerror = () => reject(open.error);
				open.onsuccess = () => {
					const read = open.result.transaction('games').objectStore('games').get(id);
					read.onsuccess = () => {
						open.result.close();
						resolve(read.result);
					};
				};
			}),
		id
	);

export interface FakeRoom {
	/** The page's seat, its Room's players, and the moves the Room holds: what a Welcome says. */
	you: 'X' | 'O';
	players: Players;
	moves: number[];
	/** What `GET /api/rooms/<id>` answers. */
	card?: RoomCard;
	/** The rest of what a Welcome says of the Room: its option, an offer waiting, how it ended. */
	snapshot?: Partial<Omit<Snapshot, 'record' | 'players'>>;
}

export const fakeRoom = async (page: Page, room: FakeRoom) => {
	const sockets: WebSocketRoute[] = [];
	const said: ClientMsg[] = [];
	const created: NewRoom[] = [];
	const card = () =>
		room.card ?? {
			host: HOST,
			variant: 'Standard',
			rules: 'std',
			players: room.players,
			you: room.you,
			takebacks: false,
			clock: null
		};
	const welcome = (): ServerMsg => ({
		type: 'Welcome',
		you: room.you,
		snapshot: {
			record: {
				rules_version: 1,
				ruleset: SHELF[0].ruleset,
				seed: '00000000005eed00',
				moves: room.moves
			},
			players: room.players,
			takebacks: false,
			offer: null,
			draws: { X: 0, O: 0 },
			end: null,
			rematch: null,
			clock: null,
			clocks: null,
			presence: { X: null, O: null },
			...room.snapshot
		}
	});
	await page.route('/api/session', (route) => route.fulfill({ json: { handle: JOINER } }));
	await page.route('/api/rooms/*', (route) => route.fulfill({ json: card() }));
	await page.route('/api/rooms', (route) => {
		created.push(route.request().postDataJSON());
		return route.fulfill({ status: 201, json: { room: ROOM } });
	});
	await page.routeWebSocket('**/api/socket', (socket) => {
		sockets.push(socket);
		socket.onMessage((data) => {
			const message: ClientMsg = JSON.parse(String(data));
			if (message.type === 'Ping') {
				const pong = { type: 'Pong', t: message.t, server_ms: Date.now() };
				return socket.send(JSON.stringify(pong));
			}
			said.push(message);
			if (message.type === 'Hello') socket.send(JSON.stringify(welcome()));
		});
	});
	const last = () => sockets[sockets.length - 1];
	return {
		said,
		/** The bodies of the Rooms the page asked for. */
		created,
		/** The moves the page sent. */
		moves: () => said.filter((message) => message.type === 'Move'),
		hellos: () => said.filter((message) => message.type === 'Hello').length,
		/** Sends `message` on the newest socket. */
		say: (message: ServerMsg) => last().send(JSON.stringify(message)),
		/** The Room's Welcome again, on the newest socket. */
		welcome: () => last().send(JSON.stringify(welcome())),
		/** `handle` takes the empty seat, and the Room welcomes the page again, as the server does. */
		join: (handle: string) => {
			room.players =
				room.players.X === null ? { ...room.players, X: handle } : { ...room.players, O: handle };
			last().send(JSON.stringify(welcome()));
		},
		/** A move the Room accepts, told on the newest socket, with a timed Room's clocks after it. */
		moved: (ply: number, cell: number, clocks: Clocks | null = null) => {
			room.moves = [...room.moves, cell];
			last().send(JSON.stringify({ type: 'Moved', ply, cell, clocks }));
		},
		close: (code: number) => last().close({ code }),
		sockets
	};
};
