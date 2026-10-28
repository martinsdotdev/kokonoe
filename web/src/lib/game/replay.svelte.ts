/**
 * A saved game shown a ply at a time, in the Replay viewer (spec §7.4): stepped, autoplayed, or
 * jumped to any ply, the dealt opening included. Every position is built once from the Record,
 * so a step only picks one, and no engine Game is held while the page shows them.
 */
import type { Game, Player, Status } from '#lib/engine.js';
import type { Opponent } from './launch.js';
import { dealtMoves, playedMove, seats, written, type Move, type Seat } from './moves.js';
import { openRecord } from './record.js';
import type { End } from './result.js';
import type { Variant } from './rules.js';
import { savedVariant, type SavedGame } from './saved.js';
import { positionView, range, type BoardView } from './view.js';

/** How long autoplay shows each ply. */
const TICK_MS = 1000;

/** The game after one ply: what the board, the player bars and the scoreboard show. */
export interface Moment {
	view: BoardView;
	status: Status;
	won: Record<Player, number>;
}

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
		this.variant = savedVariant(saved);
		this.opponent = saved.opponent;
		this.seats = seats(saved.opponent);
		const opening = game.opening();
		const plies = [...opening, ...game.moves()];
		this.moments = range(plies.length + 1).map((ply) => {
			// Every ply up to the game's own has a State.
			const state = game.stateAt(ply)!;
			try {
				return {
					view: positionView(state, plies[ply - 1] ?? null, opening.slice(0, ply)),
					status: state.status(),
					won: { X: state.wonCount('X'), O: state.wonCount('O') }
				};
			} finally {
				state.free();
			}
		});
		this.moves = [
			...dealtMoves(opening, this.variant.ruleset.depth),
			...plies.slice(opening.length).map((cell, i) => {
				const n = opening.length + i + 1;
				return playedMove(n, cell, this.moments[n - 1].view, this.moments[n].view);
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

	/** Each player's count of won Boards at the end, which a count result quotes. */
	get won(): Record<Player, number> {
		return this.moments[this.last].won;
	}

	/** The ply shown, in words: "Start. X moves first.", "Move 14 of 55 · X 5·1·8 · Won board 5". */
	get caption(): string {
		const move = this.moves[this.ply - 1];
		if (!move) return 'Start. X moves first.';
		const what = move.dealt ? 'dealt' : move.note;
		return [`Move ${move.n} of ${this.last}`, `${move.player} ${written(move)}`, what]
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
