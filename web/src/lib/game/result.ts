import type { Outcome, Player, Ruleset } from '#lib/engine.js';

/** How a game ended: the engine's Outcome, or a resignation, which only the screen records. */
export type End = { outcome: Outcome } | { resigned: Player };

export const opponent = (player: Player): Player => (player === 'X' ? 'O' : 'X');

export const winner = (end: End): Player | null => {
	if ('resigned' in end) return opponent(end.resigned);
	const { outcome } = end;
	return outcome === 'Draw' ? null : 'Objective' in outcome ? outcome.Objective : outcome.MostWon;
};

/**
 * The result banner's words (spec §7.4): "X wins on a line", "O wins on count, 5–4", "Draw".
 * `won` is each player's count of won Boards, which the count results quote.
 */
export const resultText = (end: End, ruleset: Ruleset, won: Record<Player, number>): string => {
	if ('resigned' in end) return `${end.resigned} resigned. ${opponent(end.resigned)} wins.`;
	const player = winner(end);
	if (player === null) return ruleset.fallback === 'MostWon' ? `Draw, ${won.X}–${won.O}` : 'Draw';
	if (ruleset.objective === 'Line' && end.outcome !== 'Draw' && 'Objective' in end.outcome) {
		return `${player} wins on a line`;
	}
	return `${player} wins on count, ${won[player]}–${won[opponent(player)]}`;
};
