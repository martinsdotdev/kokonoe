import { describe, expect, it, vi } from 'vitest';
import { render } from 'vitest-browser-svelte';
import SteppedBoard from './SteppedBoard.svelte';
import { position } from './view.fixtures.js';
import { at } from './view.js';

/** Sent to `space`, after the move that sent there. */
const sentTo = (space: number, last: number) =>
	position((v) => {
		v.cells[last] = 'X';
		v.last = last;
		v.sent = [space];
		at(v, [space]).active = true;
	});

type Screen = Awaited<ReturnType<typeof render>>;

const focusOf = (screen: Screen) => screen.container.querySelector<HTMLElement>('[data-focus]')!;

const settled = async (screen: Screen) =>
	expect.element(focusOf(screen)).toHaveAttribute('data-settled');

describe('following the game', () => {
	it('shows the Sent target, with its Cells to play', async () => {
		const screen = await render(SteppedBoard, { view: sentTo(4, 40) });
		await expect.element(focusOf(screen)).toHaveAttribute('data-focus', '4');
		await expect.element(screen.getByText('Board 5', { exact: true })).toBeVisible();
		expect(screen.getByRole('button', { name: /^Cell 5·/ }).all()).toHaveLength(9);
		await expect
			.element(screen.getByRole('button', { name: 'Cell 5·1', exact: true }))
			.toHaveAttribute('aria-disabled', 'false');
	});

	it('shows the Global view for the first move and a Free move, where Boards take the taps', async () => {
		const free = position((v) => {
			at(v, [0]).status = { Won: 'X' };
			v.sent = [0];
			v.within = [];
			for (const space of [1, 2, 3, 4, 5, 6, 7, 8]) at(v, [space]).active = true;
		});
		const screen = await render(SteppedBoard, { view: free });
		await expect.element(focusOf(screen)).toHaveAttribute('data-focus', 'global');
		expect(screen.getByRole('button', { name: /^Cell / }).all()).toHaveLength(0);
		await expect.element(screen.getByRole('button', { name: 'Board 1, won by X' })).toBeVisible();
		await expect.element(screen.getByRole('button', { name: 'Board 2, play here' })).toBeVisible();
	});

	it('steps in on a tap and back out by the breadcrumb, until the game moves on', async () => {
		const screen = await render(SteppedBoard, { view: position() });
		await screen.getByRole('button', { name: 'Board 3' }).click();
		await expect.element(focusOf(screen)).toHaveAttribute('data-focus', '2');
		await settled(screen);
		await screen.getByRole('button', { name: 'Global', exact: true }).click();
		await expect.element(focusOf(screen)).toHaveAttribute('data-focus', 'global');
		await settled(screen);
		await screen.getByRole('button', { name: 'Board 7' }).click();
		await expect.element(focusOf(screen)).toHaveAttribute('data-focus', '6');
		await screen.rerender({ view: sentTo(1, 64) });
		await expect.element(focusOf(screen)).toHaveAttribute('data-focus', '1');
	});

	it('holds the computer’s move in view, then follows', async () => {
		// The computer played 5·3 in Board 5, which Sends the player to Board 3.
		const screen = await render(SteppedBoard, { view: sentTo(2, 38), replied: true });
		await expect.element(focusOf(screen)).toHaveAttribute('data-focus', '4');
		await expect.element(focusOf(screen), { timeout: 2000 }).toHaveAttribute('data-focus', '2');
	});

	it('ends the hold on a step', async () => {
		const screen = await render(SteppedBoard, { view: sentTo(2, 38), replied: true });
		await settled(screen);
		await screen.getByRole('button', { name: 'Global', exact: true }).click();
		await expect.element(focusOf(screen)).toHaveAttribute('data-focus', 'global');
		await new Promise((resolve) => setTimeout(resolve, 800));
		await expect.element(focusOf(screen)).toHaveAttribute('data-focus', 'global');
	});

	it('steps to the Board a refused tap names', async () => {
		const screen = await render(SteppedBoard, { view: sentTo(4, 40) });
		await screen.rerender({ refusal: { board: [7], tick: 1 } });
		await expect.element(focusOf(screen)).toHaveAttribute('data-focus', '7');
	});
});

describe('taps', () => {
	it('wait until a step has settled', async () => {
		const onplay = vi.fn();
		const screen = await render(SteppedBoard, { view: position(), onplay });
		await screen.getByRole('button', { name: 'Board 5' }).click();
		// A double tap: the second lands at once, on the Board stepping in.
		await screen.getByRole('button', { name: 'Cell 5·5', exact: true }).click({ force: true });
		expect(onplay).not.toHaveBeenCalled();
		await settled(screen);
		await screen.getByRole('button', { name: 'Cell 5·5', exact: true }).click({ force: true });
		expect(onplay.mock.calls).toEqual([[40]]);
	});
});

describe('keyboard focus', () => {
	it('stays on the same Cell when the view follows from Board to Board', async () => {
		const screen = await render(SteppedBoard, { view: sentTo(4, 40) });
		const cell = screen.getByRole('button', { name: 'Cell 5·1', exact: true }).element();
		(cell as HTMLElement).focus();
		await screen.rerender({ view: sentTo(0, 36) });
		await expect.element(focusOf(screen)).toHaveAttribute('data-focus', '0');
		expect(document.activeElement).toBe(cell);
		expect(cell.getAttribute('aria-label')).toBe('Cell 1·1');
	});

	it('moves to the Board stepped into, and back to its button on the way out', async () => {
		const screen = await render(SteppedBoard, { view: position() });
		await screen.getByRole('button', { name: 'Board 5' }).click();
		await expect.element(focusOf(screen)).toHaveAttribute('data-focus', '4');
		expect(document.activeElement?.textContent?.trim()).toBe('Board 5');
		await settled(screen);
		await screen.getByRole('button', { name: 'Global', exact: true }).click();
		await expect.element(focusOf(screen)).toHaveAttribute('data-focus', 'global');
		expect(document.activeElement?.getAttribute('aria-label')).toBe('Board 5');
	});
});
