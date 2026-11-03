import type { Outcome, Player, Ruleset, Status } from '#lib/engine.js';

/**
 * A game stopped before its rules ended it: a resignation, an agreed draw or a player out of time,
 * which no Record says.
 */
export type Stopped = { resigned: Player } | { agreed: true } | { flagged: Player };

/** How a game ended: the engine's Outcome, or it stopped. */
export type End = { outcome: Outcome } | Stopped;

/** How a game ended: as it stopped, if it did, or as its rules ended it. */
export const endOf = (stopped: Stopped | null, status: Status): End | null =>
	stopped ?? ('Over' in status ? { outcome: status.Over } : null);

/**
 * How a game stopped, by spec §4.4's Result and Termination: an agreed draw, or a resignation or a
 * loss on time by the player who didn't win. Null for any other end, or a stop that names no
 * winner.
 */
export const stoppedFrom = (
	result: string | null | undefined,
	termination: string | undefined
): Stopped | null => {
	if (termination === 'agreed draw') return { agreed: true };
	const loser = result === '1-0' ? 'O' : result === '0-1' ? 'X' : null;
	if (!loser) return null;
	if (termination === 'resign') return { resigned: loser };
	return termination === 'timeout' ? { flagged: loser } : null;
};

export const opponent = (player: Player): Player => (player === 'X' ? 'O' : 'X');

export const winner = (end: End): Player | null => {
	if ('agreed' in end) return null;
	if ('resigned' in end) return opponent(end.resigned);
	if ('flagged' in end) return opponent(end.flagged);
	const { outcome } = end;
	return outcome === 'Draw' ? null : 'Objective' in outcome ? outcome.Objective : outcome.MostWon;
};

/** A game file's Result (spec §4.4): who won, a draw, or `*` while the game goes on. */
export const result = (end: End | null): '1-0' | '0-1' | '1/2-1/2' | '*' => {
	if (end === null) return '*';
	const player = winner(end);
	return player === null ? '1/2-1/2' : player === 'X' ? '1-0' : '0-1';
};

/**
 * The result banner's words (spec §7.4): "X wins on a line", "O wins on count, 5–4", "Draw".
 * `won` is each player's count of won Boards, which the count results quote.
 */
export const resultText = (end: End, ruleset: Ruleset, won: Record<Player, number>): string => {
	if ('agreed' in end) return 'Draw agreed';
	if ('resigned' in end) return `${end.resigned} resigned. ${opponent(end.resigned)} wins.`;
	if ('flagged' in end) return `${end.flagged} ran out of time. ${opponent(end.flagged)} wins.`;
	const player = winner(end);
	if (player === null) return ruleset.fallback === 'MostWon' ? `Draw, ${won.X}–${won.O}` : 'Draw';
	if (ruleset.objective === 'Line' && end.outcome !== 'Draw' && 'Objective' in end.outcome) {
		return `${player} wins on a line`;
	}
	return `${player} wins on count, ${won[player]}–${won[opponent(player)]}`;
};
