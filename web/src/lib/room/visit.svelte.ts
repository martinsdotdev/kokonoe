import {
	CloseCode,
	loadEngine,
	type Players,
	type Role,
	type RoomCard,
	type ServerMsg
} from '#lib/engine.js';
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

/** Why a page that asked for the free seat watches instead. */
export const TAKEN = 'Someone else took the seat first, so you’re watching.';

/** Where a visit to a Room's link stands. */
export type Phase =
	| { kind: 'loading' }
	/** The server can't be reached, and a Room is played on it: the game as this page last saw it. */
	| { kind: 'offline'; saved: SavedGame | undefined }
	| { kind: 'missing' }
	/** Both seats are taken, and the Room can't be watched. */
	| { kind: 'full'; players: Players }
	/** A seat is free: the joiner's card, until they accept it or watch. */
	| { kind: 'join'; card: RoomCard }
	| { kind: 'connecting' }
	/** Welcomed: waiting for the other player, playing, watching, or reconnecting. */
	| { kind: 'playing'; game: RoomGame }
	/**
	 * Nobody made a first move in time, and the server forgot the game: `late` didn't, "You" or the
	 * other player's Handle, or null when this page didn't see the abort.
	 */
	| { kind: 'aborted'; late: string | null }
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
 * has a seat or accepts a free one, the socket and the game. A visitor with no seat in a Room
 * that can be watched may watch it instead, as a Spectator (spec §10.2): at once if both seats are
 * taken, or by choice if one is free. Offline, it waits to be back online.
 */
export class RoomVisit {
	phase: Phase = $state.raw({ kind: 'loading' });
	readonly room: string;
	#options: VisitOptions;
	#store: ReadingStore;
	#window: Window;
	#link: Link | null = null;
	#game: RoomGame | null = null;
	/** What this page's socket comes to the Room as. */
	#role: Role = 'Player';
	/** The free seat this page asked for was taken first, so it watches: it falls back once. */
	#fellBack = false;
	/** The game as this page last saved it, if it played here before. */
	#saved: Promise<SavedGame | undefined>;
	/** The game as this page last saved it, read before the socket opens. */
	#kept: SavedGame | undefined;
	#disposed = false;

	constructor(room: string, options: VisitOptions = {}) {
		this.room = room;
		this.#options = options;
		this.#window = options.window ?? window;
		this.#store = options.store ?? browserStore;
		this.#window.addEventListener('online', this.#online);
		// A store that never answers, such as one waiting on an older tab, mustn't keep the Room shut.
		const unread = new Promise<undefined>((resolve) => setTimeout(resolve, SAVED_MS));
		const saved = this.#store.load(room).catch(() => undefined);
		this.#saved = Promise.race([saved, unread]);
		this.#start();
	}

	/** Takes the free seat the card offers. */
	accept() {
		if (this.phase.kind === 'join') this.#connect('Player');
	}

	/** Watches the game the card offers instead of taking its free seat. */
	watch() {
		if (this.phase.kind === 'join' && this.phase.card.spectators) this.#connect('Spectator');
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
		else if (card === 'missing') await this.#missing();
		else if (card.you) this.#connect('Player');
		else if (!full(card.players)) this.phase = { kind: 'join', card };
		else if (card.spectators) this.#connect('Spectator');
		else this.phase = { kind: 'full', players: card.players };
	}

	async #connect(role: Role) {
		this.#role = role;
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
		this.#link = new Link(hello(this.room, role), events, { open: this.#options.open });
	}

	#heard(message: ServerMsg) {
		const link = this.#link;
		if (!link) return;
		if (message.type === 'Aborted') return this.#aborted(this.#late());
		if (message.type !== 'Welcome') return this.#game?.receive(message);
		try {
			if (this.#game) this.#game.welcome(message);
			else {
				const options = {
					room: this.room,
					store: this.#store,
					saved: this.#kept,
					onrematch: this.#options.follow,
					notice: this.#fellBack ? TAKEN : undefined
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
		if (code === CloseCode.NoRoom) await this.#missing();
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

	/**
	 * The server didn't let the socket in. A player was most likely beaten to the seat, and then
	 * watches if the Room can be watched, once; a Spectator's refusal has no such reason, and
	 * shows as a problem, so neither can loop.
	 */
	async #refused() {
		const card = await roomCard(this.room, this.#options.fetch);
		if (this.#disposed) return;
		const taken =
			this.#role === 'Player' && card && card !== 'missing' && !card.you && full(card.players);
		if (taken && card.spectators && !this.#fellBack) {
			this.#fellBack = true;
			return this.#connect('Spectator');
		}
		this.phase = taken
			? { kind: 'full', players: card.players }
			: { kind: 'problem', message: 'Kokonoe’s server didn’t let this page into the game.' };
	}

	/**
	 * There's no such Room. Only an abort deletes one, so a game this page played and didn't see
	 * finish was aborted while it wasn't looking.
	 */
	async #missing() {
		const game = this.#game;
		const unfinished = game ? !game.end : (await this.#saved)?.result === null;
		if (this.#disposed) return;
		if (unfinished) this.#aborted(null);
		else this.phase = { kind: 'missing' };
	}

	/** Who didn't make their first move: whoever was to move when the game aborted. */
	#late(): string | null {
		const game = this.#game;
		const player = game?.toMove;
		if (!game || !player) return null;
		return player === game.you ? 'You' : game.players[player];
	}

	/**
	 * The game was aborted and the server has forgotten it. The socket goes first, before the
	 * server's close can reconnect into its 4004, and this page's copy goes too, so Home doesn't
	 * offer it again.
	 */
	#aborted(late: string | null) {
		this.#link?.dispose();
		this.#store
			.delete(this.room)
			.catch((error: unknown) => console.error('The aborted game couldn’t be deleted.', error));
		this.phase = { kind: 'aborted', late };
	}

	#online = () => this.retry();
}
