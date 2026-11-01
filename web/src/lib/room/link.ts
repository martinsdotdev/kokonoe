/**
 * A Room's socket, kept open (spec §10.3): it says `Hello` each time it connects, reads what the
 * server sends, and reconnects with backoff when the socket drops, at once when the browser comes
 * back online or the page comes back into view. A reconnect gets a fresh `Welcome`, so nothing is
 * replayed. The server's closes for another build (4001), a socket not allowed (4003) and no such
 * Room (4004) end it.
 */
import { CloseCode, type ClientMsg, type ServerMsg } from '#lib/engine.js';
import { readServerMsg } from './protocol.js';

/** What a Link needs of a WebSocket, so tests can bring their own. */
export interface SocketLike {
	send(data: string): void;
	close(): void;
	onopen: ((event: Event) => void) | null;
	onmessage: ((event: MessageEvent) => void) | null;
	onclose: ((event: CloseEvent) => void) | null;
}

export interface LinkEvents {
	/** What the server said. */
	message(message: ServerMsg): void;
	/** The socket dropped, and the Link is reconnecting. */
	dropped(): void;
	/** The server closed the socket for good, with this code. */
	ended(code: number): void;
}

export interface LinkOptions {
	/** Opens a socket to `url`: a WebSocket unless a test brings its own. */
	open?: (url: string) => SocketLike;
	/** The waits before each reconnect in turn, in milliseconds, the last repeating. */
	delays?: readonly number[];
	/** A number in [0, 1), to spread the waits. */
	random?: () => number;
	/** Where `online` and `visibilitychange` come from. */
	window?: Pick<Window, 'addEventListener' | 'removeEventListener'> & {
		document: Pick<Document, 'visibilityState'>;
	};
}

/** Each wait is up to a quarter shorter or longer, so a server's restart isn't met all at once. */
const DELAYS = [250, 500, 1000, 2000, 4000, 8000, 15000];

/** A deploy's drain (1012) waits this long first, since the server isn't back yet. */
const RESTARTING_MS = 1000;

/** The closes that end the Link: reconnecting would only meet them again. */
const FINAL: readonly number[] = [CloseCode.UpdateRequired, CloseCode.Forbidden, CloseCode.NoRoom];

export class Link {
	#hello: ClientMsg;
	#events: LinkEvents;
	#dial: (url: string) => SocketLike;
	#delays: readonly number[];
	#random: () => number;
	#window: NonNullable<LinkOptions['window']>;
	#socket: SocketLike | null = null;
	#ready = false;
	#tries = 0;
	#timer: ReturnType<typeof setTimeout> | undefined;
	#disposed = false;

	constructor(hello: ClientMsg, events: LinkEvents, options: LinkOptions = {}) {
		this.#hello = hello;
		this.#events = events;
		this.#dial = options.open ?? ((url) => new WebSocket(url));
		this.#delays = options.delays ?? DELAYS;
		this.#random = options.random ?? Math.random;
		this.#window = options.window ?? window;
		this.#window.addEventListener('online', this.#now);
		this.#window.addEventListener('visibilitychange', this.#visible);
		this.#connect();
	}

	/** Sends `message` if the socket is open; false if it isn't, and the message is gone. */
	send(message: ClientMsg): boolean {
		if (!this.#ready || !this.#socket) return false;
		this.#socket.send(JSON.stringify(message));
		return true;
	}

	/** Reconnects at once, for a fresh `Welcome`: the page missed something. */
	resync() {
		if (this.#disposed) return;
		this.#drop();
		this.#connect();
	}

	dispose() {
		this.#disposed = true;
		clearTimeout(this.#timer);
		this.#window.removeEventListener('online', this.#now);
		this.#window.removeEventListener('visibilitychange', this.#visible);
		this.#drop();
	}

	#connect() {
		clearTimeout(this.#timer);
		const scheme = location.protocol === 'https:' ? 'wss' : 'ws';
		const socket = this.#dial(`${scheme}://${location.host}/api/socket`);
		this.#socket = socket;
		socket.onopen = () => {
			this.#ready = true;
			socket.send(JSON.stringify(this.#hello));
		};
		socket.onmessage = ({ data }) => {
			const message = readServerMsg(data);
			// A frame this build can't read: perhaps a newer server, so back off before asking again.
			if (!message) return this.#lost(0);
			if (message.type === 'Welcome') this.#tries = 0;
			this.#events.message(message);
		};
		socket.onclose = ({ code }) => {
			if (FINAL.includes(code)) {
				// Disposed first, so nothing the page does next can reconnect.
				this.dispose();
				return this.#events.ended(code);
			}
			this.#lost(code === CloseCode.Restarting ? RESTARTING_MS : 0);
		};
	}

	/** Forgets the socket, closing it if it's still open, so nothing it says is heard. */
	#drop() {
		const socket = this.#socket;
		this.#socket = null;
		this.#ready = false;
		if (!socket) return;
		socket.onopen = socket.onmessage = socket.onclose = null;
		socket.close();
	}

	/** The socket is gone: say so, and reconnect after the next wait, `least` at the least. */
	#lost(least: number) {
		this.#drop();
		this.#events.dropped();
		this.#retry(least);
	}

	#retry(least: number) {
		const delays = this.#delays;
		const base = delays[Math.min(this.#tries, delays.length - 1)];
		this.#tries += 1;
		const wait = Math.max(least, base * (0.75 + this.#random() / 2));
		this.#timer = setTimeout(() => this.#connect(), wait);
	}

	/** Back online, or back in view: a Link waiting to reconnect, with no socket, tries now. */
	#now = () => {
		if (this.#disposed || this.#socket) return;
		this.#connect();
	};

	#visible = () => {
		if (this.#window.document.visibilityState === 'visible') this.#now();
	};
}
