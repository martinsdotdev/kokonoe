import {
	Game,
	type Cell,
	type ClientMsg,
	type Outcome,
	type Player,
	type Players,
	type Record as GameRecord,
	type ServerMsg,
	type Status
} from '#lib/engine.js';
import type { GameFile } from '#lib/game/gamefile.js';
import { dayOf } from '#lib/game/listing.js';
import { againstLabel } from '#lib/game/launch.js';
import { dealtMoves, gameFile, playedMove, seats, type Move, type Seat } from '#lib/game/moves.js';
import { openRecord } from '#lib/game/record.js';
import { Refusals } from '#lib/game/refusal.svelte.js';
import { resultText, type End } from '#lib/game/result.js';
import { rulesetCode, type Variant } from '#lib/game/rules.js';
import {
	browserStore,
	type GameStore,
	type RoomOpponent,
	type SavedGame
} from '#lib/game/saved.js';
import type { GameSession } from '#lib/game/screen.js';
import { presetByCode } from '#lib/game/shelf.js';
import { boardView, type BoardView } from '#lib/game/view.js';
import { full } from './api.js';

export interface RoomOptions {
	/** The Room's id, which the saved game is kept by. */
	room: string;
	/** Where the game keeps itself: IndexedDB unless a test brings its own. */
	store?: GameStore;
	/** When the game began, if this page saved it before. */
	started?: number;
}

/** What a Room's game needs of its socket. */
export interface RoomLink {
	/** False when the socket isn't open, and the message is gone. */
	send(message: ClientMsg): boolean;
	/** Reconnects for a fresh `Welcome`. */
	resync(): void;
}

type Welcome = Extract<ServerMsg, { type: 'Welcome' }>;

/** An Outcome has one key, so its JSON is one spelling. */
const sameOutcome = (a: Outcome, b: Outcome) => JSON.stringify(a) === JSON.stringify(b);

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
 */
export class RoomGame implements GameSession {
	readonly variant: Variant;
	readonly controls = null;
	/** The seat this page plays. */
	you: Player;
	/** Who holds each seat, by Handle. */
	players: Players;
	view: BoardView;
	status: Status;
	moves: Move[];
	won: Record<Player, number>;
	replied = $state.raw(false);
	#connected = $state.raw(true);
	#pending: { ply: number; cell: Cell } | null = $state.raw(null);
	#refused = new Refusals();
	#game: Game;
	#link: RoomLink;
	#room: string;
	#store: GameStore;
	#started: number;
	/** A save has succeeded, so the Replay can open it. */
	#stored = $state.raw(false);

	/** The game a first `Welcome` holds. Throws an UnplayableGame when its Record can't be played. */
	constructor(welcome: Welcome, link: RoomLink, options: RoomOptions) {
		const { record, players } = welcome.snapshot;
		const { game, moves } = replay(record);
		this.#link = link;
		this.#room = options.room;
		this.#store = options.store ?? browserStore;
		this.#started = options.started ?? Date.now();
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
				if (!('Over' in this.status && sameOutcome(this.status.Over, message.outcome))) {
					this.#resync();
				}
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
		return 'Over' in this.status ? { outcome: this.status.Over } : null;
	}

	get toMove(): Player | null {
		return 'ToMove' in this.status ? this.status.ToMove.player : null;
	}

	get frozen(): boolean {
		return (
			this.end !== null ||
			!this.#connected ||
			!this.seated ||
			this.#pending !== null ||
			this.toMove !== this.you
		);
	}

	doing(): string | null {
		return null;
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
		this.#refresh();
		this.moves = [...this.moves, playedMove(this.moves.length + 1, cell, before, this.view)];
		this.replied = mover !== this.you;
		this.#save();
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

	/** Keeps the game, once both seats are filled and someone has moved. A failed save costs play nothing. */
	#save() {
		if (!this.seated || this.moves.every((move) => move.dealt)) return;
		const end = this.end;
		const saved: SavedGame = {
			id: this.#room,
			record: this.#game.record(),
			variant: { name: this.variant.name, code: rulesetCode(this.variant.ruleset) },
			opponent: this.#opponent(''),
			resigned: null,
			started: this.#started,
			updated: Date.now(),
			view: this.view,
			toMove: end ? null : this.toMove,
			result: end ? resultText(end, this.variant.ruleset, this.won) : null
		};
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
