/**
 * What the game screen tells the player in words (spec §7.7): where the player to move may play
 * and why, and the game's one live line, which a screen reader reads after every change.
 */
import type { Board, Player, Sent } from '#lib/engine.js';
import type { Move } from './moves.js';
import { boardLabel } from './notation.js';
import { opponent } from './result.js';

export interface Turn {
	/** A Free move, which the turn card stamps. */
	free: boolean;
	/** Where to play, naming the player, after a refused Cell if there was one. */
	line: string;
	/** Why: the move that Sent the player there. */
	why: string;
}

/**
 * Where `player` may play after `last`, in Notation: anywhere on the first move, the Sent target,
 * or on a Free move anywhere open in the Board above a decided target, which at depth 2 is the
 * Global board. At depth 3 `decidedAbove` is a decided middle Board above the target, which
 * turned the move away. `refused` is a Cell just refused, which the line names first.
 */
export const turn = (
	player: Player,
	sent: Sent,
	last: string | null,
	decidedAbove: Board | null = null,
	refused: string | null = null
): Turn => {
	const where = place(player, sent, last, decidedAbove);
	return refused ? { ...where, line: `${refused} isn’t playable. ${where.line}` } : where;
};

const place = (
	player: Player,
	sent: Sent,
	last: string | null,
	decidedAbove: Board | null
): Turn => {
	const by = `${opponent(player)}’s move ${last ?? ''}`.trim();
	if (sent === 'FirstMove') {
		return {
			free: false,
			line: `${player}: play anywhere.`,
			why: 'The first move may go in any Cell.'
		};
	}
	if ('To' in sent) {
		return {
			free: false,
			line: `${player}: play in board ${boardLabel(sent.To)}.`,
			why: `${by} Sent ${player} there.`
		};
	}
	const { target, within } = sent.Free;
	const to = `${by} Sent ${player} to board ${boardLabel(target)}`;
	if (within.length) {
		const board = boardLabel(within);
		return {
			free: true,
			line: `${player}: free move, play anywhere in board ${board}.`,
			why: `${to}, which is decided, so every open board in board ${board} is ${player}’s.`
		};
	}
	const inside = decidedAbove ? `, inside board ${boardLabel(decidedAbove)},` : ',';
	return {
		free: true,
		line: `${player}: free move, play in any open board.`,
		why: `${to}${inside} which is decided, so every open board is ${player}’s.`
	};
};

/**
 * The live line: `last`, the move just made, then `next`, where to play or the result, as a
 * sentence. Naming the move keeps two turns Sent to the same Board from reading the same, which
 * a screen reader could skip.
 */
export const announcement = (last: Move | undefined, next: string): string => {
	const said = /[.!?]$/.test(next) ? next : `${next}.`;
	if (!last) return said;
	return `${last.player} ${last.dealt ? 'was dealt' : 'played'} ${last.notation}. ${said}`;
};
