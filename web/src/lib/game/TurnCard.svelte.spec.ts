import { describe, expect, it } from 'vitest';
import { render } from 'vitest-browser-svelte';
import TurnCard from './TurnCard.svelte';

describe('the turn', () => {
	it('lets the first move go anywhere', async () => {
		const screen = await render(TurnCard, { player: 'X', sent: 'FirstMove', last: null });
		await expect.element(screen.getByText('X: play anywhere.')).toBeVisible();
	});

	it('names the Sent target in Notation', async () => {
		const screen = await render(TurnCard, { player: 'O', sent: { To: [2] }, last: '5·3' });
		await expect.element(screen.getByText('O: play in board 3.')).toBeVisible();
		await expect.element(screen.getByText('X’s move 5·3 Sent O there.')).toBeVisible();
		expect(screen.container.querySelector('[data-stamp]')).toBeNull();
	});

	it('stamps a Free move', async () => {
		const screen = await render(TurnCard, {
			player: 'X',
			sent: { Free: { target: [0], within: [] } },
			last: '6·1'
		});
		await expect.element(screen.getByText('FREE MOVE')).toBeVisible();
		await expect.element(screen.getByText('X: play in any open board.')).toBeVisible();
	});

	it('says when a tap wasn’t playable, and where to play', async () => {
		const screen = await render(TurnCard, {
			player: 'O',
			sent: { To: [2] },
			last: '5·3',
			refused: true
		});
		await expect.element(screen.getByText('Not playable. O: play in board 3.')).toBeVisible();
	});
});
