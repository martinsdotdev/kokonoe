/**
 * A player away from a Room (spec §11), and the Claim the other player may make once they've been
 * away long enough, as the game screen shows it and its live line says it. Engine-free.
 */
import type { ClaimKind } from '#lib/engine.js';

/** Ending the game against a player who left: the win or a draw, from `due` on. */
export interface Claim {
	/** The other player's Handle. */
	readonly other: string;
	/** From when the server allows it, in `Date.now()`'s time. */
	readonly due: number;
	/** The connection is open, and no Claim waits for the server's answer. */
	readonly enabled: boolean;
	run(kind: ClaimKind): void;
}

export const leftText = (other: string): string => `${other} left the game.`;

export const backText = (other: string): string => `${other} is back.`;

/**
 * How many watch a Room as Spectators (spec §10.2), as its players read it, a count and never
 * names, or as a Spectator does, who is one of them.
 */
export const watchingText = (count: number, watching: boolean): string => {
	if (!watching) return `${count} watching`;
	const others = Math.max(0, count - 1);
	if (others === 0) return 'You’re watching';
	return `You’re watching, with ${others} ${others === 1 ? 'other' : 'others'}`;
};

/** What a Spectator's page says while a Room waits for its second player. */
export const UNSEATED = 'Waiting for a second player.';

/** What the Claim is, once it's due. */
export const claimText = (other: string): string =>
	`${leftText(other)} You can claim the win or a draw, or keep waiting.`;
