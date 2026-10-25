import { describe, expect, it } from 'vitest';
import { boardLabel, cellPath, notation } from './notation.js';

describe('Notation', () => {
	it('writes each Space plus 1, from the Global board down', () => {
		expect(notation(0, 2)).toBe('1·1');
		expect(notation(40, 2)).toBe('5·5');
		expect(notation(80, 2)).toBe('9·9');
		expect(notation(4 * 81 + 0 * 9 + 7, 3)).toBe('5·1·8');
		expect(cellPath(38, 2)).toEqual([4, 2]);
	});

	it('labels Boards the same way, and the Global board not at all', () => {
		expect(boardLabel([3])).toBe('4');
		expect(boardLabel([0, 7])).toBe('1·8');
		expect(boardLabel([])).toBe('');
	});
});
