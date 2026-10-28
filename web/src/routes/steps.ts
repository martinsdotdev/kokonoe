/** Playing on the board that steps into one Board at a time: on a phone, and for Deep. */
import { expect, type Locator, type Page } from '@playwright/test';
import { notation, parse } from '../lib/game/games.fixtures.js';
import { boardName } from '../lib/game/notation.js';

export const stepped = (page: Page) => page.locator('[data-focus]');

/** Waits for the view to stop stepping, and reports the Board it shows, by path. */
export const settledFocus = async (page: Page): Promise<number[]> => {
	await expect(stepped(page)).toHaveAttribute('data-settled');
	const focus = await stepped(page).getAttribute('data-focus');
	return focus === 'global' ? [] : [...focus!].map(Number);
};

/** The button that steps into `board`. */
export const stepInto = (page: Page, board: number[]) =>
	page.getByRole('button', { name: new RegExp(`^${boardName(board)}(,|$)`) });

/** The longest path both start with. */
const commonPrefix = (a: number[], b: number[]) => {
	const n = a.findIndex((space, i) => space !== b[i]);
	return a.slice(0, n === -1 ? a.length : n);
};

/**
 * Plays `moves`, pressing each Board on the way down, a level at a time, then the Cell. Before
 * each move it checks that the view followed the game: into the Local board the last move Sent
 * the player to, to the Board a Free move may go anywhere in, or out to the Global view. Under
 * Continuation off that's as much of the move's way down as it shares with the Sent target.
 * Returns the presses each move took.
 */
export const stepThrough = async (
	page: Page,
	moves: string,
	press: (target: Locator) => Promise<void>
): Promise<number[]> => {
	const played = parse(moves);
	const presses: number[] = [];
	for (const [i, path] of played.entries()) {
		const local = path.slice(0, -1);
		const sent = i === 0 ? [] : played[i - 1].slice(1);
		const shown = commonPrefix(sent, local);
		expect(await settledFocus(page), `before move ${i + 1}`).toEqual(shown);
		for (let level = shown.length + 1; level <= local.length; level++) {
			await press(stepInto(page, local.slice(0, level)));
			await settledFocus(page);
		}
		await press(page.getByRole('button', { name: `Cell ${notation(path)}`, exact: true }));
		presses.push(local.length - shown.length + 1);
	}
	return presses;
};

/** How many moves took each number of presses. */
export const tally = (presses: number[]) => {
	const counts: Record<number, number> = {};
	for (const n of presses) counts[n] = (counts[n] ?? 0) + 1;
	return counts;
};
