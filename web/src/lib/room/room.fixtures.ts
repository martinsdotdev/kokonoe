/** Fakes for the Room's unit tests: sockets a test plays the server on, and its messages. */
import type { Players, Record, ServerMsg } from '#lib/engine.js';
import { SHELF } from '#lib/game/shelf.js';
import type { SocketLike } from './link.js';

export const ROOM = '6f1c0a9b3e2d4c5a7b8e';

export const record = (moves: number[] = []): Record => ({
	rules_version: 1,
	ruleset: SHELF[0].ruleset,
	seed: '00000000005eed00',
	moves
});

export const welcome = (
	you: 'X' | 'O',
	players: Players = { X: 'guest-4821', O: 'guest-1066' },
	moves: number[] = []
): Extract<ServerMsg, { type: 'Welcome' }> => ({
	type: 'Welcome',
	you,
	snapshot: { record: record(moves), players }
});

/** A socket whose server is the test: it accepts, says things and closes on the test's word. */
export class FakeSocket implements SocketLike {
	/** What the page sent, as JSON. */
	sent: unknown[] = [];
	closed = false;
	onopen: ((event: Event) => void) | null = null;
	onmessage: ((event: MessageEvent) => void) | null = null;
	onclose: ((event: CloseEvent) => void) | null = null;

	constructor(readonly url: string) {}

	send(data: string) {
		this.sent.push(JSON.parse(data));
	}

	close() {
		this.closed = true;
	}

	accept() {
		this.onopen?.(new Event('open'));
	}

	/** Sends `message`, as JSON unless it's already text. */
	say(message: unknown) {
		const data = typeof message === 'string' ? message : JSON.stringify(message);
		this.onmessage?.(new MessageEvent('message', { data }));
	}

	end(code: number) {
		this.onclose?.(new CloseEvent('close', { code }));
	}
}

/** Every socket the page opens, newest last. */
export const sockets = () => {
	const opened: FakeSocket[] = [];
	const open = (url: string) => {
		const socket = new FakeSocket(url);
		opened.push(socket);
		return socket;
	};
	return { opened, open, last: () => opened[opened.length - 1] };
};
