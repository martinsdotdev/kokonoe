/**
 * What the game screen shows and does, whoever keeps the game: `OfflineGame` on this device, or
 * a Room's game on the server. Engine-free, so the screen imports no session's code.
 */
import type { Board, Cell, Player, Status } from '#lib/engine.js';
import type { GameFile } from './gamefile.js';
import type { Move, Seat } from './moves.js';
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
	/** The Board to flash, with a tick so the same board can flash twice running. */
	readonly flash: { board: Board; tick: number } | null;
	/** The last Cell tapped or typed was refused, until the next change. */
	readonly refusal: { cell: Cell; board: Board | null; tick: number } | null;
	/** The last change was the other side's move, which the phone board shows landing. */
	readonly replied: boolean;
	/** The board takes no moves now: the game is over, or the move isn't this side's to make. */
	readonly frozen: boolean;
	/** What a player is doing, for their bar: "Thinking…". */
	doing(player: Player): string | null;
	/** What keeps the game from going on, said under the player bars, and a way to try again. */
	readonly trouble: { message: string; retry?: () => void } | null;
	/** The saved game's id, once it's saved, for the result banner's Replay link. */
	readonly replay: string | undefined;
	/** The game as a file holds it, to export. */
	readonly file: GameFile;
	/** Plays `cell` for this side, or flashes the Board the refusal names. */
	play(cell: Cell): void;
	/** Undo and Resign, when the game has them. */
	readonly controls: Controls | null;
	/** A new game of the same kind, when the game can start one. */
	restart?(): void;
}

export interface Controls {
	readonly canUndo: boolean;
	/** Who would resign: the player to move, or the player against the computer. */
	readonly resigner: Player | null;
	undo(): void;
	resign(): void;
}
