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

describe('the turn in Deep', () => {
	it('names a Local board in three-digit Notation', async () => {
		const screen = await render(TurnCard, { player: 'O', sent: { To: [4, 7] }, last: '3·5·8' });
		await expect.element(screen.getByText('O: play in board 5·8.')).toBeVisible();
	});

	it('frees a move inside the middle Board above a decided target', async () => {
		const screen = await render(TurnCard, {
			player: 'X',
			sent: { Free: { target: [7, 6], within: [7] } },
			last: '1·8·7',
			decided: [7, 6]
		});
		await expect.element(screen.getByText('FREE MOVE')).toBeVisible();
		await expect.element(screen.getByText('X: play anywhere in board 8.')).toBeVisible();
		await expect
			.element(
				screen.getByText(
					'O’s move 1·8·7 Sent X to board 8·7, which is decided, so every open board in board 8 is X’s.'
				)
			)
			.toBeVisible();
	});

	it('names the decided middle Board when the move may go anywhere', async () => {
		const screen = await render(TurnCard, {
			player: 'X',
			sent: { Free: { target: [4, 7], within: [] } },
			last: '2·5·8',
			decided: [4]
		});
		await expect.element(screen.getByText('X: play in any open board.')).toBeVisible();
		await expect
			.element(
				screen.getByText(
					'O’s move 2·5·8 Sent X to board 5·8, inside board 5, which is decided, so every open board is X’s.'
				)
			)
			.toBeVisible();
	});
});
