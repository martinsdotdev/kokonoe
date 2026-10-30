import { describe, expect, it } from 'vitest';
import { announcement, turn } from './announce.js';
import type { Move } from './moves.js';

const move = (player: 'X' | 'O', notation: string, dealt = false): Move => ({
	n: 1,
	player,
	notation,
	dealt,
	note: ''
});

describe('the turn line', () => {
	it('names the player and where to play', () => {
		expect(turn('X', 'FirstMove', null).line).toBe('X: play anywhere.');
		expect(turn('O', { To: [2] }, '5·3')).toEqual({
			free: false,
			line: 'O: play in board 3.',
			why: 'X’s move 5·3 Sent O there.'
		});
	});

	it('says a Free move is one, anywhere or inside a middle Board', () => {
		expect(turn('X', { Free: { target: [0], within: [] } }, '6·1').line).toBe(
			'X: free move, play in any open board.'
		);
		expect(turn('X', { Free: { target: [7, 6], within: [7] } }, '1·8·7')).toMatchObject({
			free: true,
			line: 'X: free move, play anywhere in board 8.'
		});
	});

	it('names a refused Cell before where to play instead', () => {
		expect(turn('O', { To: [3] }, '2·4', null, '1·1').line).toBe(
			'1·1 isn’t playable. O: play in board 4.'
		);
	});
});

describe('the live line', () => {
	it('names the last move before where to play next', () => {
		expect(announcement(move('O', '5·3'), 'X: play in board 3.')).toBe(
			'O played 5·3. X: play in board 3.'
		);
		expect(announcement(undefined, 'X: play anywhere.')).toBe('X: play anywhere.');
	});

	it('says a dealt move was dealt', () => {
		expect(announcement(move('X', '8·1·3', true), 'O: play in board 1·3.')).toBe(
			'X was dealt 8·1·3. O: play in board 1·3.'
		);
	});

	it('ends the result as a sentence', () => {
		expect(announcement(move('X', '3·9'), 'X wins on a line')).toBe(
			'X played 3·9. X wins on a line.'
		);
		expect(announcement(undefined, 'X resigned. O wins.')).toBe('X resigned. O wins.');
	});
});
