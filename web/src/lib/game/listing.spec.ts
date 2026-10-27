import { describe, expect, it } from 'vitest';
import { detailLine, playedWhen, turnLine } from './listing.js';
import type { SavedGame } from './saved.js';

const at = (iso: string) => new Date(iso).getTime();

const game = (opponent: SavedGame['opponent'], toMove: SavedGame['toMove']) =>
	({
		opponent,
		toMove,
		variant: { name: 'Tic-Tac-Ku', code: 'cont+first5+most' },
		plies: 12
	}) as SavedGame;

describe('a saved game in words', () => {
	it('says when it was played', () => {
		const now = at('2026-10-27T18:00:00');
		expect(playedWhen(at('2026-10-27T14:05:00'), now)).toBe('Today, 14:05');
		expect(playedWhen(at('2026-10-26T09:12:00'), now)).toBe('Yesterday, 09:12');
		expect(playedWhen(at('2026-10-02T23:59:00'), now)).toBe('2 Oct');
		expect(playedWhen(at('2025-12-31T10:00:00'), now)).toBe('31 Dec 2025');
	});

	it('says whose move it is', () => {
		const computer = { kind: 'computer', level: 3, seat: 'O' } as const;
		expect(turnLine(game(computer, 'O'))).toBe('Your move against the computer');
		expect(turnLine(game(computer, 'X'))).toBe('The computer’s move');
		expect(turnLine(game({ kind: 'hotseat' }, 'X'))).toBe('X to move · pass and play');
	});

	it('names the variant, the Level and the move', () => {
		expect(detailLine(game({ kind: 'computer', level: 3, seat: 'X' }, 'X'))).toBe(
			'Tic-Tac-Ku · Level 3 · move 12'
		);
		expect(detailLine(game({ kind: 'hotseat' }, 'X'))).toBe('Tic-Tac-Ku · move 12');
	});
});
