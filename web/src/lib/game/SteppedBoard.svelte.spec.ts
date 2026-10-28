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

describe('Deep', () => {
	/** The Cell at a path of three Spaces. */
	const cell = (m: number, l: number, c: number) => m * 81 + l * 9 + c;

	/** X's move at `last` Sent O to the Local board it names, or under a Free move, `within`. */
	const deep = (last: [number, number, number], within: number[] | null = null) =>
		position((v) => {
			v.cells[cell(...last)] = 'X';
			v.last = cell(...last);
			v.sent = last.slice(1);
			v.within = within;
		}, 3);

	it('follows into the Local board Sent to, the middle Board a Free move is inside, or out', async () => {
		const screen = await render(SteppedBoard, { view: deep([0, 4, 7]) });
		await expect.element(focusOf(screen)).toHaveAttribute('data-focus', '47');
		await expect.element(screen.getByRole('heading', { name: 'Local board 5·8' })).toBeVisible();
		await expect
			.element(screen.getByRole('button', { name: 'Board 5', exact: true }))
			.toBeVisible();
		await expect
			.element(screen.getByRole('button', { name: 'Cell 5·8·1', exact: true }))
			.toBeVisible();
		await screen.rerender({ view: deep([0, 4, 7], [4]) });
		await expect.element(focusOf(screen)).toHaveAttribute('data-focus', '4');
		await expect.element(screen.getByRole('button', { name: /^Local board 5·1/ })).toBeVisible();
		await screen.rerender({ view: deep([0, 4, 7], []) });
		await expect.element(focusOf(screen)).toHaveAttribute('data-focus', 'global');
	});

	it('steps in a level a tap, and out a level a crumb', async () => {
		const screen = await render(SteppedBoard, { view: position(() => {}, 3) });
		await screen.getByRole('button', { name: 'Board 5' }).click();
		await expect.element(focusOf(screen)).toHaveAttribute('data-focus', '4');
		await settled(screen);
		await screen.getByRole('button', { name: 'Local board 5·8' }).click();
		await expect.element(focusOf(screen)).toHaveAttribute('data-focus', '47');
		await settled(screen);
		await screen.getByRole('button', { name: 'Board 5', exact: true }).click();
		await expect.element(focusOf(screen)).toHaveAttribute('data-focus', '4');
		await settled(screen);
		await screen.getByRole('button', { name: 'Global', exact: true }).click();
		await expect.element(focusOf(screen)).toHaveAttribute('data-focus', 'global');
	});

	it('holds the computer’s move in its Local board, then follows', async () => {
		const screen = await render(SteppedBoard, { view: deep([4, 4, 2]), replied: true });
		await expect.element(focusOf(screen)).toHaveAttribute('data-focus', '44');
		await expect.element(focusOf(screen), { timeout: 2000 }).toHaveAttribute('data-focus', '42');
	});

	it('steps to the Board a refusal names, at either level', async () => {
		const screen = await render(SteppedBoard, { view: deep([0, 4, 7]) });
		await screen.rerender({ refusal: { board: [4], tick: 1 } });
		await expect.element(focusOf(screen)).toHaveAttribute('data-focus', '4');
		await screen.rerender({ refusal: { board: [2, 1], tick: 2 } });
		await expect.element(focusOf(screen)).toHaveAttribute('data-focus', '21');
	});

	it('passes keyboard focus out a level at a time, to the Board just left', async () => {
		const screen = await render(SteppedBoard, { view: deep([0, 4, 7]) });
		await settled(screen);
		await screen.getByRole('button', { name: 'Board 5', exact: true }).click();
		await expect.element(focusOf(screen)).toHaveAttribute('data-focus', '4');
		expect(document.activeElement?.getAttribute('aria-label')).toBe('Local board 5·8');
		await settled(screen);
		await screen.getByRole('button', { name: 'Global', exact: true }).click();
		await expect.element(focusOf(screen)).toHaveAttribute('data-focus', 'global');
		expect(document.activeElement?.getAttribute('aria-label')).toBe('Board 5');
	});
});
