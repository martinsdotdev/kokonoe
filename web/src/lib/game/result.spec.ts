import { describe, expect, it } from 'vitest';
import type { Ruleset } from '#lib/engine.js';
import { resultText, stoppedFrom, winner } from './result.js';

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

	it('names a loss on time and its winner', () => {
		expect(resultText({ flagged: 'X' }, STANDARD, { X: 0, O: 0 })).toBe(
			'X ran out of time. O wins.'
		);
		expect(winner({ flagged: 'O' })).toBe('X');
	});

	it('names a Claim and an abandoned game, and who won', () => {
		const none = { X: 0, O: 0 };
		expect(resultText({ claimed: 'X' }, STANDARD, none)).toBe(
			'O left the game. X claimed the win.'
		);
		expect(resultText({ claimedDraw: true }, STANDARD, none)).toBe('Draw claimed');
		expect(resultText({ abandoned: true }, STANDARD, none)).toBe('Game abandoned');
		expect(winner({ claimed: 'O' })).toBe('O');
		expect(winner({ claimedDraw: true })).toBeNull();
		expect(winner({ abandoned: true })).toBeNull();
	});
});

describe('a stop, from its Result and Termination', () => {
	it('reads a resignation, an agreed draw and a loss on time', () => {
		expect(stoppedFrom('0-1', 'resign')).toEqual({ resigned: 'X' });
		expect(stoppedFrom('1-0', 'timeout')).toEqual({ flagged: 'O' });
		expect(stoppedFrom('1/2-1/2', 'agreed draw')).toEqual({ agreed: true });
	});

	it('reads a Claim, and an abandoned game whatever its Result', () => {
		expect(stoppedFrom('1-0', 'claim')).toEqual({ claimed: 'X' });
		expect(stoppedFrom('0-1', 'claim')).toEqual({ claimed: 'O' });
		expect(stoppedFrom('1/2-1/2', 'claim')).toEqual({ claimedDraw: true });
		expect(stoppedFrom(null, 'abandoned'), 'as the server says it').toEqual({ abandoned: true });
		expect(stoppedFrom('*', 'abandoned'), 'as a file says it').toEqual({ abandoned: true });
	});

	it('reads no stop from the rules’ ends, or from one that names no winner', () => {
		expect(stoppedFrom('1-0', 'line')).toBeNull();
		expect(stoppedFrom('1/2-1/2', 'timeout')).toBeNull();
		expect(stoppedFrom('*', 'resign')).toBeNull();
		expect(stoppedFrom('*', 'claim')).toBeNull();
		expect(stoppedFrom(undefined, 'claim')).toBeNull();
	});
});
