import { expect, it, vi } from 'vitest';
import { render } from 'vitest-browser-svelte';
import ResultBanner from './ResultBanner.svelte';
import { SHELF } from './shelf.js';

const banner = { headline: 'X wins on a line', detail: 'After 9 moves.', result: '1-0' } as const;

it('offers the game’s Replay and a new game', async () => {
	const onrestart = vi.fn();
	const screen = await render(ResultBanner, { ...banner, game: 'a', onrestart });
	await expect
		.element(screen.getByRole('link', { name: 'Replay' }))
		.toHaveAttribute('href', '/replay?game=a');
	await screen.getByRole('button', { name: 'New game' }).click();
	expect(onrestart).toHaveBeenCalled();
});

it('marks the winner, a draw, or an abandoned game’s want of a result', async () => {
	const disc = (result: '1-0' | '1/2-1/2' | '*') =>
		render(ResultBanner, { ...banner, result }).container.querySelector('[data-disc]')?.textContent;
	expect(disc('1-0')).toBe('X');
	expect(disc('1/2-1/2')).toBe('=');
	expect(disc('*')).toBe('–');
});

it('offers nothing in the Replay viewer, only the result', async () => {
	const screen = await render(ResultBanner, banner);
	await expect.element(screen.getByRole('heading', { name: 'X wins on a line' })).toBeVisible();
	expect(screen.getByRole('button').all()).toHaveLength(0);
	expect(screen.getByRole('link').all()).toHaveLength(0);
});

it('offers the game as a file when given one', async () => {
	const file = {
		record: { rules_version: 1, ruleset: SHELF[0].ruleset, seed: '00000000005eed00', moves: [] },
		variant: 'Standard',
		seats: { X: 'Player 1', O: 'Player 2' },
		date: '',
		opening: [],
		end: null
	};
	const screen = await render(ResultBanner, { ...banner, file });
	await expect.element(screen.getByRole('group', { name: 'Export' })).toBeVisible();
	await expect.element(screen.getByRole('button', { name: 'Export as JSON' })).toBeVisible();
});

const rematch = (enabled: boolean) => ({
	offer: { enabled, run: vi.fn() },
	waiting: null,
	onanswer: vi.fn(),
	room: null
});

it('offers a Room’s rematch while it can be offered', async () => {
	const open = rematch(true);
	const screen = await render(ResultBanner, { ...banner, rematch: open });
	await screen.getByRole('button', { name: 'Rematch' }).click();
	expect(open.offer.run).toHaveBeenCalledOnce();
	const shut = rematch(false);
	await screen.rerender({ ...banner, rematch: shut });
	const button = screen.getByRole('button', { name: 'Rematch' });
	await expect.element(button).toHaveAttribute('aria-disabled', 'true');
	await button.click({ force: true });
	expect(shut.offer.run).not.toHaveBeenCalled();
});

it('shows the other player’s rematch offer, and the rematch once made', async () => {
	const waiting = { kind: 'Rematch', mine: false, other: 'guest-1066', lapses: null } as const;
	const offered = { ...rematch(false), waiting };
	const screen = await render(ResultBanner, { ...banner, rematch: offered });
	await screen.getByRole('button', { name: 'Accept' }).click();
	expect(offered.onanswer).toHaveBeenCalledWith(true);
	const room = 'b'.repeat(20);
	await screen.rerender({ ...banner, rematch: { ...rematch(false), room } });
	await expect
		.element(screen.getByRole('link', { name: 'Play the rematch' }))
		.toHaveAttribute('href', `/r/${room}`);
	expect(screen.getByRole('button', { name: 'Rematch' }).all()).toHaveLength(0);
});
