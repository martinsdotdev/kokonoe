import { describe, expect, it } from 'vitest';
import { nearest, typed, typedSoFar, type Box } from './keys.js';

describe('typing a Cell', () => {
	it('plays at depth 2 on the second digit', () => {
		expect(typed('', '5', 2)).toEqual({ digits: '5' });
		expect(typed('5', '5', 2)).toEqual({ digits: '', cell: 40 });
	});

	it('plays in Deep on the third digit', () => {
		expect(typed('5', '1', 3)).toEqual({ digits: '51' });
		// 5·1·8 is Spaces 4, 0, 7: 4·81 + 0·9 + 7.
		expect(typed('51', '8', 3)).toEqual({ digits: '', cell: 331 });
	});

	it('takes back a digit, or clears them', () => {
		expect(typed('51', 'Backspace', 3)).toEqual({ digits: '5' });
		expect(typed('51', 'Escape', 3)).toEqual({ digits: '' });
	});

	it('leaves other keys, and Backspace or Escape with nothing typed, to the page', () => {
		for (const key of ['0', 'a', 'Enter', 'ArrowLeft']) expect(typed('5', key, 2)).toBeNull();
		expect(typed('', 'Backspace', 2)).toBeNull();
		expect(typed('', 'Escape', 2)).toBeNull();
	});

	it('shows what’s typed with a gap for each digit to come', () => {
		expect(typedSoFar('', 2)).toBe('_·_');
		expect(typedSoFar('5', 2)).toBe('5·_');
		expect(typedSoFar('51', 3)).toBe('5·1·_');
	});
});

describe('moving between buttons with the arrows', () => {
	/** A 3 by 3 grid of 10 by 10 boxes, a gap of 2 between them, in reading order. */
	const grid: Box[] = Array.from({ length: 9 }, (_, i) => ({
		x: (i % 3) * 12,
		y: Math.floor(i / 3) * 12,
		width: 10,
		height: 10
	}));

	it('moves to the neighbour that way', () => {
		expect(nearest(grid[4], grid, 'ArrowRight')).toBe(5);
		expect(nearest(grid[4], grid, 'ArrowLeft')).toBe(3);
		expect(nearest(grid[4], grid, 'ArrowUp')).toBe(1);
		expect(nearest(grid[4], grid, 'ArrowDown')).toBe(7);
	});

	it('stays at an edge', () => {
		expect(nearest(grid[2], grid, 'ArrowRight')).toBe(-1);
		expect(nearest(grid[6], grid, 'ArrowDown')).toBe(-1);
	});

	it('crosses a wider gap, as between Boards', () => {
		const apart: Box[] = [grid[0], { ...grid[1], x: 40 }];
		expect(nearest(apart[0], apart, 'ArrowRight')).toBe(1);
	});

	it('ignores other keys', () => {
		expect(nearest(grid[4], grid, 'Enter')).toBe(-1);
	});
});
