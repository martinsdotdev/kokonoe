import type { Ruleset } from '#lib/engine.js';

/**
 * The Ruleset code (spec §4.2): `std` for Standard, otherwise the settings that differ from
 * Standard joined by `+` in the fixed order `d3`, `cont`, `both`, `first<N>`, `most`, `rand<K>`.
 * Formatting only: KOK-34 adds the engine's parser.
 */
export const rulesetCode = (rules: Ruleset): string => {
	const tokens = [
		rules.depth === 3 && 'd3',
		rules.continuation === 'On' && 'cont',
		rules.drawn_owner === 'Both' && 'both',
		rules.objective !== 'Line' && `first${rules.objective.FirstTo}`,
		rules.fallback === 'MostWon' && 'most',
		rules.opening !== 'Free' && `rand${rules.opening.Random}`
	].filter((token) => typeof token === 'string');
	return tokens.length > 0 ? tokens.join('+') : 'std';
};

export interface RuleRow {
	label: string;
	value: string;
}

/** The six settings in plain words, for the New game sheet's summary and the game screen. */
export const describeRules = (rules: Ruleset): RuleRow[] => {
	const { objective, opening } = rules;
	return [
		{
			label: 'Boards',
			value: rules.depth === 2 ? 'Two levels, 81 Cells' : 'Three levels, 729 Cells'
		},
		{
			label: 'To win',
			value:
				objective === 'Line'
					? 'Three won boards in a line'
					: `First to ${objective.FirstTo} won ${objective.FirstTo === 1 ? 'board' : 'boards'}`
		},
		{
			label: 'Won boards',
			value:
				rules.continuation === 'Off' ? 'Closed to further moves' : 'Keep taking moves until full'
		},
		{
			label: 'Drawn boards count for',
			value: rules.drawn_owner === 'Neither' ? 'Nobody' : 'Both players'
		},
		{
			label: 'If nobody gets there',
			value: rules.fallback === 'Draw' ? 'A draw' : 'Most boards won wins; equal counts draw'
		},
		{
			label: 'Opening',
			value:
				opening === 'Free'
					? 'Free: X moves first, anywhere'
					: opening.Random === 1
						? 'The first move is dealt at random'
						: `The first ${opening.Random} moves are dealt at random`
		}
	];
};
