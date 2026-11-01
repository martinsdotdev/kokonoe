import {
	Game,
	type Cell,
	type ClientMsg,
	type Draws,
	type GameEnd,
	type Offer,
	type OfferKind,
	type Player,
	type Players,
	type Record as GameRecord,
	type ServerMsg,
	type Status
} from '#lib/engine.js';
import type { GameFile } from '#lib/game/gamefile.js';
import { dayOf } from '#lib/game/listing.js';
import { againstLabel, type RoomOpponent } from '#lib/game/launch.js';
import { dealtMoves, gameFile, playedMove, seats, type Move, type Seat } from '#lib/game/moves.js';
import { openRecord } from '#lib/game/record.js';
import { Refusals } from '#lib/game/refusal.svelte.js';
import { DRAWS, OFFERS, unansweredText, type PendingOffer } from '#lib/game/offers.js';
import { endOf, opponent, result, stoppedFrom, type End, type Stopped } from '#lib/game/result.js';
import { rulesetCode, type Variant } from '#lib/game/rules.js';
import { browserStore, savedGame, type GameStore, type SavedGame } from '#lib/game/saved.js';
import type { Action, Controls, GameSession } from '#lib/game/screen.js';
import { presetByCode } from '#lib/game/shelf.js';
import { boardView, type BoardView } from '#lib/game/view.js';
import { full } from './api.js';

export interface RoomOptions {
	/** The Room's id, which the saved game is kept by. */
	room: string;
	/** Where the game keeps itself: IndexedDB unless a test brings its own. */
	store?: GameStore;
	/** The game as this page last saved it, if it played here before. */
	saved?: SavedGame;
	/** Goes to the Room a rematch made, once both players agreed to it. */
	onrematch?: (room: string) => void;
}

/** What a Room's game needs of its socket. */
export interface RoomLink {
	/** False when the socket isn't open, and the message is gone. */
	send(message: ClientMsg): boolean;
	/** Reconnects for a fresh `Welcome`. */
	resync(): void;
}

type Welcome = Extract<ServerMsg, { type: 'Welcome' }>;
type Snapshot = Welcome['snapshot'];

/** What a save holds that the Room can change: the moves, who sits where, and the result. */
const kept = (saved: SavedGame): string =>
	JSON.stringify([saved.record.moves, saved.opponent, saved.result]);

/** What a page sends to make an offer of each kind, and to answer one. */
const OFFER = { Draw: 'OfferDraw', Takeback: 'AskTakeback', Rematch: 'OfferRematch' } as const;
const ANSWER = {
	Draw: 'AnswerDraw',
	Takeback: 'AnswerTakeback',
	Rematch: 'AnswerRematch'
} as const;

/**
 * The game a Record leads to, replayed move by move so the move list has its notes. Throws an
 * UnplayableGame when this version of Kokonoe can't play it.
 */
const replay = (record: GameRecord): { game: Game; moves: Move[] } => {
	openRecord(record).free();
	const game = new Game(record.ruleset, record.seed);
	let moves = dealtMoves(game.opening(), record.ruleset.depth);
	for (const cell of record.moves) {
		const before = boardView(game);
		game.play(game.ply(), cell);
		moves = [...moves, playedMove(moves.length + 1, cell, before, boardView(game))];
	}
	return { game, moves };
};

/**
 * A Room's game, as this page knows it (spec §10.3). The server holds the game: this copy is
 * rebuilt from every `Welcome`, and moves only on `Moved`, in ply order. A tap the engine refuses
 * flashes at once, as offline; one it allows is sent, and the board waits for the server's
 * `Moved` before showing it. Anything out of step, a gap in plies or a refusal, reconnects for a
 * fresh `Welcome`, and the board waits for that.
 *
 * Once both seats are filled it keeps itself after every move it hears, so Home can return to the
 * Room and, once it's over, replay it offline.
 *
 * The Room's policies are the server's (spec §10.2): this page sends a resignation, an offer or an
 * answer, and shows what the server says came of it. Its own offer shows at once, since a refusal
 * resyncs.
 */
export class RoomGame implements GameSession {
	readonly variant: Variant;
	/** The seat this page plays. */
	you: Player;
	/** Who holds each seat, by Handle. */
	players: Players;
	view: BoardView;
	status: Status;
	moves: Move[];
	won: Record<Player, number>;
	replied = $state.raw(false);
	/** This page's offer came to nothing: the other player declined it, or let it lapse. */
	notice: string | null = $state.raw(null);
	#takebacks = $state.raw(false);
	#draws: Draws = $state.raw({ X: 0, O: 0 });
	#offer: (Offer & { lapses: number | null }) | null = $state.raw(null);
	#stopped: Stopped | null = $state.raw(null);
	#rematch: string | null = $state.raw(null);
	#onrematch: ((room: string) => void) | undefined;
	#connected = $state.raw(true);
	#pending: { ply: number; cell: Cell } | null = $state.raw(null);
	#refused = new Refusals();
	#game: Game;
	#link: RoomLink;
	#room: string;
	#store: GameStore;
	#started: number;
	/** What the last save held, so a `Welcome` that changes nothing isn't saved again. */
	#kept: string | null;
	/** A save has succeeded, so the Replay can open it. */
	#stored = $state.raw(false);

	/** The game a first `Welcome` holds. Throws an UnplayableGame when its Record can't be played. */
	constructor(welcome: Welcome, link: RoomLink, options: RoomOptions) {
		const { record, players } = welcome.snapshot;
		const { game, moves } = replay(record);
		this.#link = link;
		this.#room = options.room;
		this.#store = options.store ?? browserStore;
		this.#onrematch = options.onrematch;
		this.#started = options.saved?.started ?? Date.now();
		this.#kept = options.saved ? kept(options.saved) : null;
		this.#stored = options.saved !== undefined;
		this.#game = game;
		this.variant = {
			name: presetByCode(rulesetCode(record.ruleset))?.name ?? 'Custom variant',
			ruleset: record.ruleset
		};
		this.you = $state.raw(welcome.you);
		this.players = $state.raw(players);
		this.moves = $state.raw(moves);
		this.view = $state.raw(boardView(game));
		this.status = $state.raw(game.status());
		this.won = $state.raw(this.#counts());
		this.#take(welcome.snapshot);
		this.#save();
	}

	/** Everything a later `Welcome` holds, in place of what the page knew. */
	welcome(welcome: Welcome) {
		const { game, moves } = replay(welcome.snapshot.record);
		this.#game.free();
		this.#game = game;
		this.you = welcome.you;
		this.players = welcome.snapshot.players;
		this.moves = moves;
		this.#pending = null;
		this.#connected = true;
		this.#take(welcome.snapshot);
		this.#refresh();
		this.#save();
	}

	/** The socket dropped: no moves until the next `Welcome`. */
	dropped() {
		this.#connected = false;
	}

	/** What the server said, other than a `Welcome`. */
	receive(message: Exclude<ServerMsg, Welcome>) {
		switch (message.type) {
			case 'Moved':
				return this.#moved(message.ply, message.cell);
			case 'Rejected':
				// The page's engine allowed what the server's refused: they disagree.
				return this.#resync();
			case 'Ended':
				return this.#ended(message);
			case 'Offer':
				return this.#offered(message);
			case 'OfferAnswered':
				return this.#answered(message.kind, message.accepted);
			case 'TookBack':
				return this.#tookBack(message.to_ply);
			case 'RematchRoom':
				this.#rematch = message.room;
				this.#onrematch?.(message.room);
		}
	}

	get connected(): boolean {
		return this.#connected;
	}

	/** The move sent and not yet heard back, which the board waits for. */
	get pending() {
		return this.#pending;
	}

	get flash() {
		return this.#refused.flash;
	}

	get refusal() {
		return this.#refused.refusal;
	}

	get seats(): Record<Player, Seat> {
		return seats(this.#opponent('Waiting…'));
	}

	get against(): string {
		return againstLabel(this.#opponent('a friend'));
	}

	/** The saved game's id once it's kept, for the Replay. */
	get replay(): string | undefined {
		return this.#stored ? this.#room : undefined;
	}

	/** Both seats are taken, so the game can begin. */
	get seated(): boolean {
		return full(this.players);
	}

	get end(): End | null {
		return endOf(this.#stopped, this.status);
	}

	get toMove(): Player | null {
		return 'ToMove' in this.status ? this.status.ToMove.player : null;
	}

	get frozen(): boolean {
		return !this.#live || this.end !== null || this.#pending !== null || this.toMove !== this.you;
	}

	doing(): string | null {
		return null;
	}

	/** Resign and the offers while the game goes on, takebacks only if the Room allows them. */
	get controls(): Controls {
		const playing = this.#live && this.end === null;
		const free = playing && this.#offer === null;
		const controls: Controls = {
			resigner: playing ? this.you : null,
			resign: () => this.#link.send({ type: 'Resign' }),
			offerDraw: { enabled: free && this.#draws[this.you] < DRAWS, run: () => this.#make('Draw') }
		};
		if (!this.#takebacks) return controls;
		// The server takes the game back to before this player's last move, so they need one.
		const moved = this.moves.some((move) => !move.dealt && move.player === this.you);
		return {
			...controls,
			askTakeback: { enabled: free && moved, run: () => this.#make('Takeback') }
		};
	}

	/** The offer waiting for an answer, this page's or the other player's. */
	get offer(): PendingOffer | null {
		const offer = this.#offer;
		if (!offer) return null;
		return {
			kind: offer.kind,
			mine: offer.by === this.you,
			other: this.#other,
			lapses: offer.lapses
		};
	}

	/** Answers the other player's offer. */
	answer(accept: boolean) {
		const offer = this.#offer;
		if (!offer || offer.by === this.you) return;
		if (this.#link.send({ type: ANSWER[offer.kind], accept })) this.#offer = null;
	}

	/** A rematch, once the game is over, while nothing waits and none was made. */
	get rematch(): Action {
		const over = this.#live && this.end !== null;
		return {
			enabled: over && this.#offer === null && this.#rematch === null,
			run: () => this.#make('Rematch')
		};
	}

	/** The Room this game's rematch made. */
	get rematchRoom(): string | null {
		return this.#rematch;
	}

	get trouble() {
		return this.#connected || this.end ? null : { message: 'Connection lost. Reconnecting…' };
	}

	get file(): GameFile {
		return gameFile({
			record: this.#game.record(),
			variant: this.variant.name,
			seats: this.seats,
			date: dayOf(this.#started),
			opening: this.#game.opening(),
			end: this.end
		});
	}

	/** Sends `cell` as this seat's move, if the engine allows it, or flashes the Board it names. */
	play(cell: Cell) {
		if (this.frozen) return;
		const ply = this.#game.ply();
		const result = this.#game.play(ply, cell);
		if (typeof result === 'string') return;
		if ('Illegal' in result) return this.#refused.refuse(cell, result.Illegal);
		if (!('Now' in result)) return;
		// The server's `Moved` plays it.
		this.#game.undo();
		if (this.#link.send({ type: 'Move', ply, cell })) this.#pending = { ply, cell };
	}

	dispose() {
		this.#refused.dispose();
		this.#game.free();
	}

	#moved(ply: number, cell: Cell) {
		const at = this.#game.ply();
		if (ply < at) return;
		if (ply > at) return this.#resync();
		const mover = this.toMove;
		const before = this.view;
		const result = this.#game.play(at, cell);
		if (typeof result === 'string' || !('Now' in result)) return this.#resync();
		this.#pending = null;
		// The server withdraws a takeback with any move, and a draw offer with its player's, saying
		// so after the move: dropped here, the offer's `OfferAnswered` doesn't read as declined.
		const offer = this.#offer;
		if (offer && (offer.kind === 'Takeback' || (offer.kind === 'Draw' && offer.by === mover))) {
			this.#offer = null;
		}
		this.notice = null;
		this.#refresh();
		this.moves = [...this.moves, playedMove(this.moves.length + 1, cell, before, this.view)];
		this.replied = mover !== this.you;
		this.#save();
	}

	/** Both seats are filled and the socket is open. */
	get #live(): boolean {
		return this.seated && this.#connected;
	}

	/** The other player's Handle. */
	get #other(): string {
		return this.players[opponent(this.you)] ?? 'your friend';
	}

	/** How the game ended, by the server's word: a stop, or the rules' end the engine reached. */
	#ended(end: GameEnd) {
		this.#settle();
		const stopped = stoppedFrom(end.result, end.termination);
		if (!stopped) {
			// The rules' end, which the engine must have reached too.
			if (result(this.end) !== end.result) this.#resync();
			return;
		}
		// A stop after the engine's end, which can't be.
		if ('Over' in this.status) return this.#resync();
		this.#stopped = stopped;
		this.#pending = null;
		this.#save();
	}

	#offered(offer: Offer) {
		if (offer.kind === 'Draw') {
			this.#draws = { ...this.#draws, [offer.by]: this.#draws[offer.by] + 1 };
		}
		const wait = OFFERS[offer.kind].lapses;
		this.#offer = { ...offer, lapses: wait === null ? null : Date.now() + wait };
		this.notice = null;
	}

	/**
	 * The offer of `kind` stands no more. Accepted, the next message says what came of it; else
	 * this page hears that its own came to nothing. One a move withdrew is already gone.
	 */
	#answered(kind: OfferKind, accepted: boolean) {
		const offer = this.#offer;
		if (offer?.kind !== kind) return;
		this.#offer = null;
		if (!accepted && offer.by === this.you) this.notice = unansweredText(kind, this.#other);
	}

	/** Both players agreed to take the game back to before ply `to`. */
	#tookBack(to: number) {
		this.#settle();
		if (to > this.#game.ply()) return this.#resync();
		while (this.#game.ply() > to) {
			if (this.#game.undo() === undefined) return this.#resync();
		}
		this.moves = this.moves.slice(0, to);
		this.#pending = null;
		this.#refresh();
		this.#save();
	}

	/** No offer waits, and nothing is left to say of one. */
	#settle() {
		this.#offer = null;
		this.notice = null;
	}

	/** Offers `kind` to the other player. */
	#make(kind: OfferKind) {
		if (!this.#link.send({ type: OFFER[kind] })) return;
		this.#offer = { kind, by: this.you, lapses: null };
		this.notice = null;
	}

	/** What a `Welcome` says of the Room beyond its game's moves. */
	#take(snapshot: Snapshot) {
		this.#takebacks = snapshot.takebacks;
		this.#draws = snapshot.draws;
		this.#offer = snapshot.offer && { ...snapshot.offer, lapses: null };
		this.#stopped = snapshot.end && stoppedFrom(snapshot.end.result, snapshot.end.termination);
		this.#rematch = snapshot.rematch;
		this.notice = null;
	}

	/** The Room as a saved game's opponent, naming an empty seat `empty`. */
	#opponent(empty: string): RoomOpponent {
		const { X, O } = this.players;
		return {
			kind: 'room',
			room: this.#room,
			you: this.you,
			players: { X: X ?? empty, O: O ?? empty }
		};
	}

	/**
	 * Keeps the game once both seats are filled and someone has moved, unless what's kept already
	 * holds it, and again after a takeback even to before every move. A failed save costs play
	 * nothing.
	 */
	#save() {
		const unplayed = this.moves.every((move) => move.dealt);
		if (!this.seated || (unplayed && this.#kept === null)) return;
		const saved = savedGame({
			id: this.#room,
			record: this.#game.record(),
			variant: this.variant,
			opponent: this.#opponent(''),
			started: this.#started,
			view: this.view,
			toMove: this.toMove,
			end: this.end,
			won: this.won
		});
		const holds = kept(saved);
		if (holds === this.#kept) return;
		this.#kept = holds;
		this.#store.save(saved).then(
			() => (this.#stored = true),
			(error: unknown) => console.error('The game couldn’t be saved.', error)
		);
	}

	/** Out of step with the server: wait, without moving, for a fresh `Welcome`. */
	#resync() {
		this.#pending = null;
		this.#connected = false;
		this.#link.resync();
	}

	#refresh() {
		this.#refused.clear();
		this.replied = false;
		this.view = boardView(this.#game);
		this.status = this.#game.status();
		this.won = this.#counts();
	}

	#counts(): Record<Player, number> {
		return { X: this.#game.wonCount('X'), O: this.#game.wonCount('O') };
	}
}
