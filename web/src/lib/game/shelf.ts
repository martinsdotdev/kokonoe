/**
 * The five Presets as Home and the New game sheet show them. Static, so neither page loads the
 * engine; `shelf.browser.spec.ts` holds each name and Ruleset equal to the engine's `presets()`.
 */
import type { Ruleset } from '#lib/engine.js';
import { rulesetCode } from './rules.js';

export interface ShelfPreset {
	name: string;
	ruleset: Ruleset;
	blurb: string;
	chips: string[];
}

const STANDARD: Ruleset = {
	depth: 2,
	continuation: 'Off',
	drawn_owner: 'Neither',
	objective: 'Line',
	fallback: 'Draw',
	opening: 'Free'
};

export const SHELF: ShelfPreset[] = [
	{
		name: 'Standard',
		ruleset: STANDARD,
		blurb: 'The classic rules. Three won boards in a row wins.',
		chips: ['Depth 2', 'Line']
	},
	{
		name: 'Forced continuation',
		ruleset: { ...STANDARD, continuation: 'On' },
		blurb: 'Won boards stay open. Play goes on inside them until they fill.',
		chips: ['Depth 2', 'Continuation']
	},
	{
		name: 'Tic-Tac-Ku',
		ruleset: { ...STANDARD, continuation: 'On', objective: { FirstTo: 5 }, fallback: 'MostWon' },
		blurb: 'First to win five boards takes it. No lines needed.',
		chips: ['First to 5', 'Most won']
	},
	{
		name: 'Count tiebreak',
		ruleset: { ...STANDARD, fallback: 'MostWon' },
		blurb: 'Standard, but a game with no line goes to whoever won more boards.',
		chips: ['Line', 'Most won']
	},
	{
		name: 'Deep',
		ruleset: { ...STANDARD, depth: 3 },
		blurb: 'One more level of nesting. 729 Cells, for the patient.',
		chips: ['Depth 3', 'Line']
	}
];

/** The Preset a Ruleset code names, if any. */
export const presetByCode = (code: string): ShelfPreset | undefined =>
	SHELF.find((preset) => rulesetCode(preset.ruleset) === code);
