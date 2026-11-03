/**
 * What the game screen shows and does, whoever keeps the game: `OfflineGame` on this device, or
 * a Room's game on the server. Engine-free, so the screen imports no session's code.
 */
import type { Cell, Player, Status } from '#lib/engine.js';
import type { ClockView } from './clock.js';
import type { GameFile } from './gamefile.js';
import type { Move, Seat } from './moves.js';
import type { PendingOffer } from './offers.js';
import type { Claim } from './presence.js';
import type { Flash, Refusal } from './refusal.svelte.js';
import type { End } from './result.js';
import type { Variant } from './rules.js';
import type { BoardView } from './view.js';

export interface GameSession {
	readonly variant: Variant;
	readonly seats: Record<Player, Seat>;
	/** Who the game is against, for the header: "Pass and play", "vs Computer · Level 3". */
	readonly against: string;
	readonly view: BoardView;
	readonly status: Status;
	readonly moves: Move[];
	readonly won: Record<Player, number>;
	readonly end: End | null;
	readonly toMove: Player | null;
	readonly flash: Flash | null;
	/** The last Cell tapped or typed was refused, until the next change. */
	readonly refusal: Refusal | null;
	/** The last change was the other side's move, which the phone board shows landing. */
	readonly replied: boolean;
	/** The board takes no moves now: the game is over, or the move isn't this side's to make. */
	readonly frozen: boolean;
	/** What a player is doing, for their bar: "Thinking…". */
	doing(player: Player): string | null;
	/**
	 * What keeps the game from going on, shown under the player bars and said by the game's live
	 * line, and a way to try again.
	 */
	readonly trouble: { message: string; retry?: () => void } | null;
	/** The saved game's id, once it's saved, for the result banner's Replay link. */
	readonly replay: string | undefined;
	/** The game as a file holds it, to export. */
	readonly file: GameFile;
	/** Plays `cell` for this side, or flashes the Board the refusal names. */
	play(cell: Cell): void;
	/** Undo, Resign and the offers, when the game has them. */
	readonly controls: Controls | null;
	/** A new game of the same kind, when the game can start one. */
	restart?(): void;
	/** The offer waiting for an answer, in a Room. */
	readonly offer: PendingOffer | null;
	/** Answers the other player's offer. */
	answer(accept: boolean): void;
	/**
	 * What came of this page's offer, or that the other player left or came back, when nothing else
	 * says.
	 */
	readonly notice: string | null;
	/** A timed Room's clocks, or null for an untimed Room or a game on this device. */
	readonly clocks: ClockView | null;
	/**
	 * Since when each player has been away from the Room, in `Date.now()`'s time: null while
	 * they're here, for an empty seat, for this page's own player, and on this device.
	 */
	readonly away: Record<Player, number | null>;
	/** The Claim this page may make against a player who left, while the game goes on. */
	readonly claim: Claim | null;
	/** Offers a Room's rematch, once its game is over. */
	readonly rematch?: Action;
	/** The Room this game's rematch made, to follow. */
	readonly rematchRoom: string | null;
}

/** Something a player can do, now or not. */
export interface Action {
	readonly enabled: boolean;
	run(): void;
}

/** What the controls can do. Undo is offline; the offers are a Room's. */
export interface Controls {
	/** Who would resign: the player to move, or this page's player. Null once the game is over. */
	readonly resigner: Player | null;
	resign(): void;
	readonly undo?: Action;
	readonly offerDraw?: Action;
	/** Only in a Room that allows takebacks. */
	readonly askTakeback?: Action;
}
