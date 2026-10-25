import { describe, expect, it } from 'vitest';
import type { Ruleset } from '#lib/engine.js';
import { resultText, winner } from './result.js';

const STANDARD: Ruleset = {
	depth: 2,
	continuation: 'Off',
	drawn_owner: 'Neither',
	objective: 'Line',
	fallback: 'Draw',
	opening: 'Free'
};
const COUNT_TIEBREAK: Ruleset = { ...STANDARD, fallback: 'MostWon' };
const TIC_TAC_KU: Ruleset = { ...COUNT_TIEBREAK, continuation: 'On', objective: { FirstTo: 5 } };

describe('the result in words', () => {
	it('names a line, a count and a draw', () => {
		const won = { X: 5, O: 4 };
		expect(resultText({ outcome: { Objective: 'X' } }, STANDARD, won)).toBe('X wins on a line');
		expect(resultText({ outcome: { Objective: 'X' } }, TIC_TAC_KU, won)).toBe(
			'X wins on count, 5–4'
		);
		expect(resultText({ outcome: { MostWon: 'O' } }, COUNT_TIEBREAK, { X: 3, O: 4 })).toBe(
			'O wins on count, 4–3'
		);
		expect(resultText({ outcome: 'Draw' }, STANDARD, won)).toBe('Draw');
		expect(resultText({ outcome: 'Draw' }, COUNT_TIEBREAK, { X: 4, O: 4 })).toBe('Draw, 4–4');
	});

	it('names a resignation and its winner', () => {
		expect(resultText({ resigned: 'X' }, STANDARD, { X: 0, O: 0 })).toBe('X resigned. O wins.');
		expect(winner({ resigned: 'X' })).toBe('O');
		expect(winner({ outcome: 'Draw' })).toBeNull();
	});
});
