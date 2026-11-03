import { describe, expect, it } from 'vitest';
import { render } from 'vitest-browser-svelte';
import PlayerBar from './PlayerBar.svelte';

const seat = { name: 'guest-4821', detail: null, yours: true };

describe('a player’s bar', () => {
	it('names the mark by its letter, which a clock takes the place of on screen', async () => {
		const untimed = render(PlayerBar, { player: 'X', seat, toMove: true });
		await expect.element(untimed.getByText('X', { exact: true })).not.toHaveClass('sr-only');
		expect(untimed.container.querySelector('[data-clock]')).toBeNull();

		const timed = render(PlayerBar, { player: 'O', seat, toMove: false, clock: 171_000 });
		await expect.element(timed.getByText('O', { exact: true })).toHaveClass('sr-only');
		await expect.element(timed.getByText('2:51')).toBeVisible();
		await expect.element(timed.getByText('2 minutes 51 seconds left')).toHaveClass('sr-only');
	});

	it('fills a clock under 20 seconds, and shows one run out', async () => {
		const clock = (ms: number) =>
			render(PlayerBar, { player: 'X', seat, toMove: true, clock: ms }).container.querySelector(
				'[data-clock]'
			);
		expect(clock(20_000)?.hasAttribute('data-low')).toBe(false);
		expect(clock(19_999)?.hasAttribute('data-low')).toBe(true);
		expect(clock(0)?.textContent).toBe('0:00.0');
	});
});
