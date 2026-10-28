/**
 * A saved game shown a ply at a time, in the Replay viewer (spec §7.4): stepped, autoplayed, or
 * jumped to any ply, the dealt opening included. Every position is built once from the Record,
 * so a step only picks one, and no engine Game is held while the page shows them.
 */
import type { Game, Player, Status } from '#lib/engine.js';
import type { Opponent } from './launch.js';
import { dealtMoves, note, seats, type Move, type Seat } from './moves.js';
import { notation } from './notation.js';
import { openRecord } from './record.js';
import type { End } from './result.js';
import type { SavedGame } from './saved.js';
import type { Variant } from './session.svelte.js';
import { positionView, type BoardView } from './view.js';

/** How long autoplay shows each ply. */
const TICK_MS = 1000;

/** The game after one ply: what the board, the player bars and the scoreboard show. */
export interface Moment {
	view: BoardView;
	status: Status;
	won: Record<Player, number>;
}

const range = (n: number) => [...Array(n).keys()];

export class Replay {
	readonly variant: Variant;
	readonly opponent: Opponent;
	readonly seats: Record<Player, Seat>;
	/** How the game ended, or null when it was left unfinished. */
	readonly end: End | null;
	/** Every ply as the move list shows it, the dealt ones first. */
	readonly moves: Move[];
	/** The game after each ply: the empty board at 0, the last ply at `moves.length`. */
	readonly moments: Moment[];
	/** The ply shown. */
	ply = $state(0);
	/** Autoplay is stepping through the plies. */
	playing = $state(false);
	#timer: ReturnType<typeof setInterval> | undefined;

	/**
	 * The Replay of a saved game. Throws an UnplayableGame when this version of Kokonoe can't
	 * replay its Record.
	 */
	static open(saved: SavedGame): Replay {
		const game = openRecord(saved.record);
		try {
			return new Replay(saved, game);
		} finally {
			game.free();
		}
	}

	private constructor(saved: SavedGame, game: Game) {
		const { ruleset } = saved.record;
		this.variant = { name: saved.variant.name, ruleset };
		this.opponent = saved.opponent;
		this.seats = seats(saved.opponent);
		const opening = game.opening();
		const plies = [...opening, ...game.moves()];
		this.moments = range(plies.length + 1).map((ply) => {
			const state = game.stateAt(ply)!;
			try {
				return {
					view: positionView(state, plies.slice(0, ply), Math.min(ply, opening.length)),
					status: state.status(),
					won: { X: state.wonCount('X'), O: state.wonCount('O') }
				};
			} finally {
				state.free();
			}
		});
		this.moves = [
			...dealtMoves(opening, ruleset.depth),
			...plies.slice(opening.length).map((cell, i): Move => {
				const n = opening.length + i + 1;
				return {
					n,
					// X moves first and turns alternate, the deal included.
					player: n % 2 ? 'X' : 'O',
					notation: notation(cell, ruleset.depth),
					dealt: false,
					note: note(this.moments[n - 1].view, this.moments[n].view, cell)
				};
			})
		];
		const last = this.moments[plies.length].status;
		this.end = saved.resigned
			? { resigned: saved.resigned }
			: 'Over' in last
				? { outcome: last.Over }
				: null;
	}

	/** The last ply. */
	get last(): number {
		return this.moves.length;
	}

	/** The game at the ply shown. */
	get shown(): Moment {
		return this.moments[this.ply];
	}

	/** The ply shown, in words: "Start. X moves first.", "Move 14 of 55 · X 5·1·8 · Won board 5". */
	get caption(): string {
		const move = this.moves[this.ply - 1];
		if (!move) return 'Start. X moves first.';
		const played = move.dealt ? `(${move.notation})` : move.notation;
		const what = move.dealt ? 'dealt' : move.note;
		return [`Move ${move.n} of ${this.last}`, `${move.player} ${played}`, what]
			.filter(Boolean)
			.join(' · ');
	}

	/** Shows `ply`, kept between the start and the last ply, and stops autoplay. */
	go(ply: number) {
		this.pause();
		this.ply = Math.min(Math.max(ply, 0), this.last);
	}

	/** Starts autoplay, from the start when the last ply is shown, or stops it. */
	toggle() {
		if (this.playing || this.last === 0) return this.pause();
		if (this.ply === this.last) this.ply = 0;
		this.playing = true;
		this.#timer = setInterval(() => {
			this.ply += 1;
			if (this.ply >= this.last) this.pause();
		}, TICK_MS);
	}

	pause() {
		clearInterval(this.#timer);
		this.playing = false;
	}

	dispose() {
		this.pause();
	}
}
