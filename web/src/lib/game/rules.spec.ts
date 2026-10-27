import { describe, expect, it } from 'vitest';
import { describeRules, rulesetCode } from './rules.js';
import { SHELF } from './shelf.js';

const preset = (name: string) => SHELF.find((p) => p.name === name)!.ruleset;

describe('Ruleset codes', () => {
	it('spell each Preset as spec §2.2 does', () => {
		expect(SHELF.map((p) => [p.name, rulesetCode(p.ruleset)])).toEqual([
			['Standard', 'std'],
			['Forced continuation', 'cont'],
			['Tic-Tac-Ku', 'cont+first5+most'],
			['Count tiebreak', 'most'],
			['Deep', 'd3']
		]);
	});

	it('join the settings that differ in the fixed order', () => {
		const custom = {
			...preset('Standard'),
			continuation: 'On',
			drawn_owner: 'Both',
			objective: { FirstTo: 5 },
			fallback: 'MostWon',
			opening: { Random: 4 }
		} as const;
		expect(rulesetCode(custom)).toBe('cont+both+first5+most+rand4');
		expect(rulesetCode({ ...custom, depth: 3 })).toBe('d3+cont+both+first5+most+rand4');
	});
});

describe('the rules in plain words', () => {
	it('describe Standard', () => {
		expect(describeRules(preset('Standard'))).toEqual([
			{ label: 'Boards', value: 'Two levels, 81 Cells' },
			{ label: 'To win', value: 'Three won boards in a line' },
			{ label: 'Won boards', value: 'Closed to further moves' },
			{ label: 'Drawn boards count for', value: 'Nobody' },
			{ label: 'If nobody gets there', value: 'A draw' },
			{ label: 'Opening', value: 'Free: X moves first, anywhere' }
		]);
	});

	it('describe Tic-Tac-Ku and the singular cases', () => {
		const rows = describeRules(preset('Tic-Tac-Ku'));
		expect(rows.map((row) => row.value)).toEqual([
			'Two levels, 81 Cells',
			'First to 5 won boards',
			'Keep taking moves until full',
			'Nobody',
			'Most boards won wins; equal counts draw',
			'Free: X moves first, anywhere'
		]);
		const one = describeRules({
			...preset('Standard'),
			objective: { FirstTo: 1 },
			opening: { Random: 1 },
			drawn_owner: 'Both'
		});
		expect(one.map((row) => row.value)).toContain('First to 1 won board');
		expect(one.map((row) => row.value)).toContain('The first move is dealt at random');
		expect(one.map((row) => row.value)).toContain('Both players');
	});
});
