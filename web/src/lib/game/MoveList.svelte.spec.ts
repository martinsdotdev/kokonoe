import { expect, it, vi } from 'vitest';
import { render } from 'vitest-browser-svelte';
import type { Move } from './moves.js';
import MoveList from './MoveList.svelte';

const moves: Move[] = [
	{ n: 1, player: 'X', notation: '5·5', dealt: true, note: '' },
	{ n: 2, player: 'O', notation: '5·1', dealt: false, note: 'Sent → 1' },
	{ n: 3, player: 'X', notation: '1·5', dealt: false, note: 'Sent → 5' }
];

const highlighted = (container: HTMLElement) =>
	[...container.querySelectorAll('li')].map((row) => row.style.background !== '');

it('lists the moves in a game, dealt ones in parentheses, the latest highlighted', async () => {
	const screen = await render(MoveList, { moves, depth: 2 });
	await expect.element(screen.getByText('(5·5)')).toBeVisible();
	expect(screen.getByRole('button').all()).toHaveLength(0);
	expect(highlighted(screen.container)).toEqual([false, false, true]);
});

it('jumps to a ply in a replay, the one shown marked as current', async () => {
	const onjump = vi.fn();
	const screen = await render(MoveList, { moves, depth: 2, current: 2, onjump });
	expect(highlighted(screen.container)).toEqual([false, true, false]);
	await expect
		.element(screen.getByRole('button', { name: /^2 O 5·1/ }))
		.toHaveAttribute('aria-current', 'step');
	await screen.getByRole('button', { name: /^1 X \(5·5\)/ }).click();
	expect(onjump).toHaveBeenCalledWith(1);

	await screen.rerender({ moves, depth: 2, current: 0, onjump });
	expect(highlighted(screen.container), 'the start highlights none').toEqual([false, false, false]);
});
