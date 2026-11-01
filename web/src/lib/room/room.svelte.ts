import {
	Game,
	type Board,
	type Cell,
	type ClientMsg,
	type Outcome,
	type Player,
	type Players,
	type ServerMsg,
	type Status
} from '#lib/engine.js';
import type { GameFile } from '#lib/game/gamefile.js';
import { dayOf } from '#lib/game/listing.js';
import { dealtMoves, gameFile, playedMove, type Move, type Seat } from '#lib/game/moves.js';
import { openRecord } from '#lib/game/record.js';
import { FLASH_MS, named } from '#lib/game/refusal.js';
import type { End } from '#lib/game/result.js';
import { rulesetCode, type Variant } from '#lib/game/rules.js';
import type { GameSession } from '#lib/game/screen.js';
import { presetByCode } from '#lib/game/shelf.js';
import { boardView, type BoardView } from '#lib/game/view.js';

/** What a Room's game needs of its socket. */
export interface RoomLink {
	/** False when the socket isn't open, and the message is gone. */
	send(message: ClientMsg): boolean;
	/** Reconnects for a fresh `Welcome`: the game missed something. */
	resync(): void;
}

type Welcome = Extract<ServerMsg, { type: 'Welcome' }>;

const opponent = (player: Player): Player => (player === 'X' ? 'O' : 'X');

const sameOutcome = (a: Outcome, b: Outcome) => JSON.stringify(a) === JSON.stringify(b);

/**
 * A Room's game, as this page knows it (spec §10.3). The server holds the game: this copy is
 * rebuilt from every `Welcome`, and moves only on `Moved`, in ply order. A tap the engine refuses
 * flashes at once, as offline; one it allows is sent, and the board waits for the server's
 * `Moved` before showing it. Anything out of step, a gap in plies or a refusal, reconnects for a
 * fresh `Welcome`.
 */
export class RoomGame implements GameSession {
	readonly variant: Variant;
	readonly controls = null;
	readonly replay = undefined;
	/** The seat this page plays. */
	you: Player = $state.raw('X');
	/** Who holds each seat, by Handle. */
	players: Players = $state.raw({ X: null, O: null });
	/** The socket is open and the server has welcomed it. */
	connected = $state.raw(true);
	/** The move sent and not yet heard back, which the board waits for. */
	pending: { ply: number; cell: Cell } | null = $state.raw(null);
	view: BoardView = $state.raw() as BoardView;
	status: Status = $state.raw() as Status;
	moves: Move[] = $state.raw([]);
	won: Record<Player, number> = $state.raw({ X: 0, O: 0 });
	flash: { board: Board; tick: number } | null = $state.raw(null);
	refusal: { cell: Cell; board: Board | null; tick: number } | null = $state.raw(null);
	replied = $state.raw(false);
	#game: Game;
	#link: RoomLink;
	#started = Date.now();
	#flashTimer: ReturnType<typeof setTimeout> | undefined;

	/** The game a first `Welcome` holds. Throws an UnplayableGame when its Record can't be played. */
	constructor(welcome: Welcome, link: RoomLink) {
		this.#link = link;
		const { ruleset } = welcome.snapshot.record;
		this.variant = { name: presetByCode(rulesetCode(ruleset))?.name ?? 'Custom variant', ruleset };
		this.#game = this.#open(welcome);
	}

	/** Everything a later `Welcome` holds, in place of what the page knew. */
	welcome(welcome: Welcome) {
		const game = this.#open(welcome);
		this.#game.free();
		this.#game = game;
	}

	/** What the server said, other than a `Welcome`. */
	receive(message: Exclude<ServerMsg, Welcome>) {
		switch (message.type) {
			case 'Moved':
				return this.#moved(message.ply, message.cell);
			case 'Rejected':
				// The page's engine allowed what the server's refused: they disagree.
				this.pending = null;
				return this.#link.resync();
			case 'Ended':
				if (!('Over' in this.status && sameOutcome(this.status.Over, message.outcome))) {
					this.#link.resync();
				}
		}
	}

	get seats(): Record<Player, Seat> {
		const seat = (player: Player): Seat => ({
			name: this.players[player] ?? 'Waiting…',
			detail: player === this.you ? 'You' : null
		});
		return { X: seat('X'), O: seat('O') };
	}

	get against(): string {
		return `vs ${this.players[opponent(this.you)] ?? 'a friend'}`;
	}

	/** Both seats are taken, so the game can begin. */
	get seated(): boolean {
		return this.players.X !== null && this.players.O !== null;
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
			!this.connected ||
			!this.seated ||
			this.pending !== null ||
			this.toMove !== this.you
		);
	}

	doing(): string | null {
		return null;
	}

	get trouble() {
		return this.connected || this.end ? null : { message: 'Connection lost. Reconnecting…' };
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
		if ('Illegal' in result) {
			const board = named(result.Illegal);
			this.#flash(board);
			this.refusal = { cell, board, tick: (this.refusal?.tick ?? 0) + 1 };
			return;
		}
		if (!('Now' in result)) return;
		// The server's `Moved` plays it.
		this.#game.undo();
		if (this.#link.send({ type: 'Move', ply, cell })) this.pending = { ply, cell };
	}

	dispose() {
		clearTimeout(this.#flashTimer);
		this.#game.free();
	}

	/** A `Welcome`'s game, its Record replayed move by move so the move list has its notes. */
	#open({ you, snapshot }: Welcome): Game {
		const { record } = snapshot;
		openRecord(record).free();
		const game = new Game(record.ruleset, record.seed);
		let moves = dealtMoves(game.opening(), record.ruleset.depth);
		for (const cell of record.moves) {
			const before = boardView(game);
			game.play(game.ply(), cell);
			moves = [...moves, playedMove(moves.length + 1, cell, before, boardView(game))];
		}
		this.you = you;
		this.players = snapshot.players;
		this.pending = null;
		this.moves = moves;
		this.#refresh(game);
		return game;
	}

	#moved(ply: number, cell: Cell) {
		const at = this.#game.ply();
		if (ply < at) return;
		if (ply > at) return this.#link.resync();
		const mover = this.toMove;
		const before = this.view;
		const result = this.#game.play(at, cell);
		if (typeof result === 'string' || !('Now' in result)) return this.#link.resync();
		this.pending = null;
		this.#refresh(this.#game);
		this.moves = [...this.moves, playedMove(this.moves.length + 1, cell, before, this.view)];
		this.replied = mover !== this.you;
	}

	#refresh(game: Game) {
		this.refusal = null;
		this.replied = false;
		this.view = boardView(game);
		this.status = game.status();
		this.won = { X: game.wonCount('X'), O: game.wonCount('O') };
	}

	#flash(board: Board | null) {
		clearTimeout(this.#flashTimer);
		this.flash = board === null ? null : { board, tick: (this.flash?.tick ?? 0) + 1 };
		if (board !== null) this.#flashTimer = setTimeout(() => (this.flash = null), FLASH_MS);
	}
}
