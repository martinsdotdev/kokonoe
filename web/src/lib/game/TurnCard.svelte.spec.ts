import { describe, expect, it } from 'vitest';
import { render } from 'vitest-browser-svelte';
import type { Board, Player, Sent } from '#lib/engine.js';
import { turn } from './announce.js';
import TurnCard from './TurnCard.svelte';

/** The card for `player`, Sent as `sent` by `last`, as the game screen passes it. */
const card = (
	player: Player,
	sent: Sent,
	last: string | null,
	more: { decidedAbove?: Board; refused?: string; typed?: string } = {}
) =>
	render(TurnCard, {
		player,
		turn: turn(player, sent, last, more.decidedAbove, more.refused),
		typed: more.typed
	});

describe('the turn', () => {
	it('lets the first move go anywhere', async () => {
		const screen = await card('X', 'FirstMove', null);
		await expect.element(screen.getByText('X: play anywhere.')).toBeVisible();
	});

	it('names the Sent target in Notation', async () => {
		const screen = await card('O', { To: [2] }, '5·3');
		await expect.element(screen.getByText('O: play in board 3.')).toBeVisible();
		await expect.element(screen.getByText('X’s move 5·3 Sent O there.')).toBeVisible();
		expect(screen.container.querySelector('[data-stamp]')).toBeNull();
	});

	it('stamps a Free move', async () => {
		const screen = await card('X', { Free: { target: [0], within: [] } }, '6·1');
		await expect.element(screen.getByText('FREE MOVE', { exact: true })).toBeVisible();
		await expect.element(screen.getByText('X: free move, play in any open board.')).toBeVisible();
	});

	it('says which Cell wasn’t playable, and where to play', async () => {
		const screen = await card('O', { To: [2] }, '5·3', { refused: '5·1' });
		await expect.element(screen.getByText('5·1 isn’t playable. O: play in board 3.')).toBeVisible();
	});

	it('shows a Cell being typed', async () => {
		const screen = await card('X', 'FirstMove', null, { typed: '5·_' });
		await expect.element(screen.getByText('Typing 5·_')).toBeVisible();
	});
});

describe('the turn in Deep', () => {
	it('names a Local board in three-digit Notation', async () => {
		const screen = await card('O', { To: [4, 7] }, '3·5·8');
		await expect.element(screen.getByText('O: play in board 5·8.')).toBeVisible();
	});

	it('frees a move inside the middle Board above a decided target', async () => {
		const screen = await card('X', { Free: { target: [7, 6], within: [7] } }, '1·8·7');
		await expect.element(screen.getByText('FREE MOVE', { exact: true })).toBeVisible();
		await expect.element(screen.getByText('X: free move, play anywhere in board 8.')).toBeVisible();
		await expect
			.element(
				screen.getByText(
					'O’s move 1·8·7 Sent X to board 8·7, which is decided, so every open board in board 8 is X’s.'
				)
			)
			.toBeVisible();
	});

	it('names the decided middle Board when the move may go anywhere', async () => {
		const screen = await card('X', { Free: { target: [4, 7], within: [] } }, '2·5·8', {
			decidedAbove: [4]
		});
		await expect.element(screen.getByText('X: free move, play in any open board.')).toBeVisible();
		await expect
			.element(
				screen.getByText(
					'O’s move 2·5·8 Sent X to board 5·8, inside board 5, which is decided, so every open board is X’s.'
				)
			)
			.toBeVisible();
	});
});
