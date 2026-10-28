import { expect, it, vi } from 'vitest';
import { render } from 'vitest-browser-svelte';
import ResultBanner from './ResultBanner.svelte';
import { SHELF } from './shelf.js';

const result = { headline: 'X wins on a line', detail: 'After 9 moves.', winner: 'X' } as const;

it('offers the game’s Replay and a new game', async () => {
	const onrestart = vi.fn();
	const screen = await render(ResultBanner, { ...result, game: 'a', onrestart });
	await expect
		.element(screen.getByRole('link', { name: 'Replay' }))
		.toHaveAttribute('href', '/replay?game=a');
	await screen.getByRole('button', { name: 'New game' }).click();
	expect(onrestart).toHaveBeenCalled();
});

it('offers nothing in the Replay viewer, only the result', async () => {
	const screen = await render(ResultBanner, result);
	await expect.element(screen.getByRole('heading', { name: 'X wins on a line' })).toBeVisible();
	expect(screen.getByRole('button').all()).toHaveLength(0);
	expect(screen.getByRole('link').all()).toHaveLength(0);
});

it('offers the game as a file when given one', async () => {
	const exported = {
		record: { rules_version: 1, ruleset: SHELF[0].ruleset, seed: '00000000005eed00', moves: [] },
		variant: 'Standard',
		seats: { X: 'Player 1', O: 'Player 2' },
		date: '',
		opening: [],
		end: null
	};
	const screen = await render(ResultBanner, { ...result, exported });
	await expect.element(screen.getByRole('group', { name: 'Export' })).toBeVisible();
	await expect.element(screen.getByRole('button', { name: 'Export as JSON' })).toBeVisible();
});
