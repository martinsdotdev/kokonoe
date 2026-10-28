import { beforeAll, describe, expect, it } from 'vitest';
import {
	loadEngine,
	notation as engineNotation,
	parseRulesetCode,
	rulesetCode as engineCode,
	type Objective,
	type Opening,
	type Ruleset
} from '#lib/engine.js';
import { notation, parseNotation } from './notation.js';
import { rulesetCode } from './rules.js';

beforeAll(() => loadEngine());

/** All 1,440 Rulesets. */
const everyRuleset = (): Ruleset[] => {
	const objectives: Objective[] = [
		'Line',
		...[...Array(9).keys()].map((n) => ({ FirstTo: n + 1 }))
	];
	const openings: Opening[] = ['Free', ...[...Array(8).keys()].map((k) => ({ Random: k + 1 }))];
	const all: Ruleset[] = [];
	for (const depth of [2, 3] as const)
		for (const continuation of ['Off', 'On'] as const)
			for (const drawn_owner of ['Neither', 'Both'] as const)
				for (const objective of objectives)
					for (const fallback of ['Draw', 'MostWon'] as const)
						for (const opening of openings)
							all.push({ depth, continuation, drawn_owner, objective, fallback, opening });
	return all;
};

describe('the display formatters, held to the engine', () => {
	it('write every Ruleset code as the engine does, and the engine reads it back', () => {
		const all = everyRuleset();
		expect(all).toHaveLength(1_440);
		for (const rules of all) {
			const code = engineCode(rules);
			expect(rulesetCode(rules)).toBe(code);
			expect(parseRulesetCode(code)).toEqual(rules);
		}
	});

	it('write every Cell in Notation as the engine does, at both depths', () => {
		for (const depth of [2, 3] as const) {
			for (let cell = 0; cell < 9 ** depth; cell++) {
				expect(notation(cell, depth)).toBe(engineNotation(cell, depth));
			}
		}
	});

	it('read every Cell back from the engine’s Notation, with `·` or `.`', () => {
		for (const depth of [2, 3] as const) {
			for (let cell = 0; cell < 9 ** depth; cell++) {
				const written = engineNotation(cell, depth);
				expect(parseNotation(written, depth)).toBe(cell);
				expect(parseNotation(written.replaceAll('·', '.'), depth)).toBe(cell);
			}
		}
		for (const text of ['5·5·5', '5', '0·1', '5·10', '5·', 'a·b', '']) {
			expect(parseNotation(text, 2), text).toBeNull();
		}
	});

	it('hear why a code is refused', () => {
		expect(() => parseRulesetCode('most+cont')).toThrow(/go in the order d3, cont, both/);
		expect(() => parseRulesetCode('first10')).toThrow(/first takes a number from 1 to 9/);
		expect(() => parseRulesetCode('Cont')).toThrow(/are std, d3, cont, both, first1 to first9/);
	});
});
