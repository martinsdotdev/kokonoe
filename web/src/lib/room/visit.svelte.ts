import { CloseCode, loadEngine, type Players, type RoomCard, type ServerMsg } from '#lib/engine.js';
import { startSession } from '#lib/account.js';
import { ENGINE } from '#lib/game/problem.js';
import { UnplayableGame } from '#lib/game/record.js';
import { browserStore, type ReadingStore, type SavedGame } from '#lib/game/saved.js';
import { updateNow } from '#lib/update.js';
import { full, isRoomId, roomCard } from './api.js';
import { Link, type LinkOptions } from './link.js';
import { hello } from './protocol.js';
import { RoomGame } from './room.svelte.js';

/** How long the page waits to read its saved copy of the game before it goes on without. */
const SAVED_MS = 1500;

/** Where a visit to a Room's link stands. */
export type Phase =
	| { kind: 'loading' }
	/** The server can't be reached, and a Room is played on it: the game as this page last saw it. */
	| { kind: 'offline'; saved: SavedGame | undefined }
	| { kind: 'missing' }
	| { kind: 'full'; players: Players }
	/** A seat is free: the joiner's card, until they accept. */
	| { kind: 'join'; card: RoomCard }
	| { kind: 'connecting' }
	/** Welcomed: waiting for the other player, playing, or reconnecting. */
	| { kind: 'playing'; game: RoomGame }
	/** The server plays a newer build, which couldn't be installed. */
	| { kind: 'update' }
	| { kind: 'problem'; message: string };

export interface VisitOptions {
	fetch?: typeof globalThis.fetch;
	open?: LinkOptions['open'];
	/** Where the game keeps itself, and was read back from: IndexedDB unless a test brings its own. */
	store?: ReadingStore;
	/** Installs the newer build and reloads into it. */
	update?: () => Promise<void>;
	/** Goes to the Room a rematch made. */
	follow?: (room: string) => void;
	window?: Window;
}

/**
 * Following a Room's link (spec §10.1): a session, then the Room's card, then, once the visitor
 * has a seat or accepts a free one, the socket and the game. Offline, it waits to be back online.
 */
export class RoomVisit {
	phase: Phase = $state.raw({ kind: 'loading' });
	readonly room: string;
	#options: VisitOptions;
	#window: Window;
	#link: Link | null = null;
	#game: RoomGame | null = null;
	/** The game as this page last saved it, if it played here before. */
	#saved: Promise<SavedGame | undefined>;
	/** The game as this page last saved it, read before the socket opens. */
	#kept: SavedGame | undefined;
	#disposed = false;

	constructor(room: string, options: VisitOptions = {}) {
		this.room = room;
		this.#options = options;
		this.#window = options.window ?? window;
		this.#window.addEventListener('online', this.#online);
		// A store that never answers, such as one waiting on an older tab, mustn't keep the Room shut.
		const unread = new Promise<undefined>((resolve) => setTimeout(resolve, SAVED_MS));
		const saved = (options.store ?? browserStore).load(room).catch(() => undefined);
		this.#saved = Promise.race([saved, unread]);
		this.#start();
	}

	/** Takes the free seat the card offers. */
	accept() {
		if (this.phase.kind === 'join') this.#connect();
	}

	/** Tries again after the server couldn't be reached. */
	retry() {
		if (this.phase.kind === 'offline') this.#start();
	}

	dispose() {
		this.#disposed = true;
		this.#window.removeEventListener('online', this.#online);
		this.#link?.dispose();
		this.#game?.dispose();
	}

	async #start() {
		// A mistyped link needs no server to say so.
		if (!isRoomId(this.room)) {
			this.phase = { kind: 'missing' };
			return;
		}
		this.phase = { kind: 'loading' };
		const fetch = this.#options.fetch ?? globalThis.fetch;
		const me = await startSession(fetch);
		const card = me && (await roomCard(this.room, fetch));
		if (this.#disposed) return;
		if (card === undefined) this.phase = { kind: 'offline', saved: await this.#saved };
		else if (card === 'missing') this.phase = { kind: 'missing' };
		else if (card.you) this.#connect();
		else if (full(card.players)) this.phase = { kind: 'full', players: card.players };
		else this.phase = { kind: 'join', card };
	}

	async #connect() {
		this.phase = { kind: 'connecting' };
		try {
			await loadEngine();
		} catch (error) {
			console.error(ENGINE, error);
			this.phase = { kind: 'problem', message: ENGINE };
			return;
		}
		// Read before the socket opens, so every message finds the game it's for.
		this.#kept = await this.#saved;
		if (this.#disposed) return;
		const events = {
			message: (message: ServerMsg) => this.#heard(message),
			dropped: () => this.#game?.dropped(),
			ended: (code: number) => this.#ended(code)
		};
		this.#link = new Link(hello(this.room), events, { open: this.#options.open });
	}

	#heard(message: ServerMsg) {
		const link = this.#link;
		if (!link) return;
		if (message.type !== 'Welcome') return this.#game?.receive(message);
		try {
			if (this.#game) this.#game.welcome(message);
			else {
				const options = {
					room: this.room,
					store: this.#options.store,
					saved: this.#kept,
					onrematch: this.#options.follow
				};
				this.#game = new RoomGame(message, link, options);
			}
		} catch (error) {
			link.dispose();
			let message = 'This game can’t be played here.';
			if (error instanceof UnplayableGame) message += ` ${error.message}`;
			else console.error('The game couldn’t open.', error);
			this.phase = { kind: 'problem', message };
			return;
		}
		this.phase = { kind: 'playing', game: this.#game };
	}

	async #ended(code: number) {
		this.#game?.dropped();
		if (code === CloseCode.NoRoom) this.phase = { kind: 'missing' };
		else if (code === CloseCode.UpdateRequired) await this.#update();
		else await this.#refused();
	}

	/** The server plays a newer build: install it and reload, or say why not. */
	async #update() {
		try {
			await (this.#options.update ?? updateNow)();
		} catch (error) {
			console.error('The newer build couldn’t be installed.', error);
			if (!this.#disposed) this.phase = { kind: 'update' };
		}
	}

	/** The server didn't let the socket in: someone else took the seat first, most likely. */
	async #refused() {
		const card = await roomCard(this.room, this.#options.fetch);
		if (this.#disposed) return;
		this.phase =
			card && card !== 'missing' && !card.you && full(card.players)
				? { kind: 'full', players: card.players }
				: { kind: 'problem', message: 'Kokonoe’s server didn’t let this page into the game.' };
	}

	#online = () => this.retry();
}
