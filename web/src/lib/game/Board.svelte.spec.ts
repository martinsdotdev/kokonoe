import { describe, expect, it, vi } from 'vitest';
import { render } from 'vitest-browser-svelte';
import type { Player } from '#lib/engine.js';
import Board from './Board.svelte';
import { at, indexOf, type BoardView } from './view.js';
import { position } from './view.fixtures.js';

/** Board 3 (Space 2), won by `player` along its top row. */
const winTopRight = (view: BoardView, player: Player) => {
	for (const inner of [0, 1, 2]) view.cells[indexOf([2, inner])] = player;
	at(view, [2]).status = { Won: player };
};

const group = (screen: Awaited<ReturnType<typeof render>>, label: string) =>
	screen.getByRole('group', { name: label, exact: true });

describe('the board grammar', () => {
	it('washes the Active boards', async () => {
		const screen = await render(Board, { view: position((v) => (at(v, [5]).active = true)) });
		await expect.element(group(screen, 'Board 6, play here')).toHaveAttribute('data-active');
		await expect.element(group(screen, 'Board 5')).not.toHaveAttribute('data-active');
	});

	it('outlines the Sent target and labels it in Notation', async () => {
		const screen = await render(Board, {
			view: position((v) => {
				v.sent = [5];
				at(v, [5]).active = true;
			})
		});
		await expect
			.element(group(screen, 'Board 6, Sent here, play here'))
			.toHaveAttribute('data-sent');
		await expect.element(screen.getByText('Sent here')).toBeVisible();
	});

	it('rings the last move', async () => {
		const screen = await render(Board, {
			view: position((v) => {
				v.cells[32] = 'O';
				v.last = 32;
			})
		});
		const cell = screen.getByRole('button', { name: 'Cell 4·6, O' });
		expect(cell.element().querySelector('[data-last]')).not.toBeNull();
	});

	it('draws dealt marks apart, and names them dealt', async () => {
		const screen = await render(Board, {
			view: position((v) => {
				v.cells[40] = 'X';
				v.cells[36] = 'O';
				v.dealt = [40];
			})
		});
		await expect
			.element(screen.getByRole('button', { name: 'Cell 5·5, X, dealt' }))
			.toHaveAttribute('data-dealt');
		await expect
			.element(screen.getByRole('button', { name: 'Cell 5·1, O', exact: true }))
			.not.toHaveAttribute('data-dealt');
	});

	it('inks a won board and fades its Cells', async () => {
		const screen = await render(Board, { view: position((v) => winTopRight(v, 'X')) });
		const board = group(screen, 'Board 3, won by X');
		await expect.element(board).toHaveAttribute('data-status', 'won-x');
		expect(board.element().querySelector('[data-owner="X"]')?.classList).not.toContain('watermark');
		expect(board.element().querySelector('.cells')?.classList).toContain('faded');
	});

	it('cross-hatches a drawn board without fading it', async () => {
		const screen = await render(Board, { view: position((v) => (at(v, [7]).status = 'Drawn')) });
		const board = group(screen, 'Board 8, drawn');
		await expect.element(board).toHaveAttribute('data-status', 'drawn');
		expect(board.element().querySelector('.hatch')).not.toBeNull();
		expect(board.element().querySelector('.cells')?.classList).not.toContain('faded');
	});

	it('watermarks a won board that still takes moves under Continuation', async () => {
		const screen = await render(Board, {
			view: position((v) => {
				v.continuation = true;
				winTopRight(v, 'O');
				at(v, [2]).active = true;
			})
		});
		const board = group(screen, 'Board 3, won by O, play here');
		await expect.element(board).toHaveAttribute('data-active');
		expect(board.element().querySelector('[data-owner="O"]')?.classList).toContain('watermark');
		expect(board.element().querySelector('.cells')?.classList).not.toContain('faded');
		await expect
			.element(screen.getByRole('button', { name: 'Cell 3·4' }))
			.toHaveAttribute('aria-disabled', 'false');
	});

	it('marks the decided target of a Free move and washes every open board', async () => {
		const screen = await render(Board, {
			view: position((v) => {
				winTopRight(v, 'X');
				v.sent = [2];
				v.within = [];
				for (const space of [0, 1, 3, 4, 5, 6, 7, 8]) at(v, [space]).active = true;
			})
		});
		const target = group(screen, 'Board 3, won by X, Sent here');
		await expect.element(target).toHaveAttribute('data-sent');
		await expect.element(target).not.toHaveAttribute('data-active');
		for (const label of ['Board 1', 'Board 5', 'Board 9']) {
			await expect.element(group(screen, `${label}, play here`)).toHaveAttribute('data-active');
		}
	});
});

describe('clicks', () => {
	it('reports the Cell clicked, legal or not', async () => {
		const onplay = vi.fn();
		const screen = await render(Board, {
			view: position((v) => (at(v, [4]).active = true)),
			onplay
		});
		await expect
			.element(screen.getByRole('button', { name: 'Cell 5·5' }))
			.toHaveAttribute('aria-disabled', 'false');
		await screen.getByRole('button', { name: 'Cell 5·5' }).click();
		// aria-disabled Cells still take clicks in a browser; Playwright needs telling.
		await screen.getByRole('button', { name: 'Cell 1·1' }).click({ force: true });
		expect(onplay.mock.calls).toEqual([[40], [0]]);
	});

	it('ignores clicks once the game is over', async () => {
		const onplay = vi.fn();
		const screen = await render(Board, { view: position(), onplay, disabled: true });
		await screen.getByRole('button', { name: 'Cell 5·5' }).click({ force: true });
		expect(onplay).not.toHaveBeenCalled();
	});

	it('flashes the Board an illegal tap names', async () => {
		const screen = await render(Board, { view: position(), flash: { board: [4], tick: 1 } });
		await expect.element(group(screen, 'Board 5')).toHaveAttribute('data-flash');
		await expect.element(group(screen, 'Board 4')).not.toHaveAttribute('data-flash');
	});
});

describe('a Deep board that steps', () => {
	const tile = (screen: Awaited<ReturnType<typeof render>>, board: string) =>
		screen.container.querySelector<HTMLElement>(`[data-board="${board}"]`)!;

	it('offers its middle Boards to step into, and no Cells', async () => {
		const onstep = vi.fn();
		const view = position((v) => {
			for (const space of [0, 1, 2, 3, 4, 5, 6, 7, 8]) at(v, [space]).active = true;
		}, 3);
		const screen = await render(Board, { view, onstep });
		expect(screen.getByRole('button').all()).toHaveLength(9);
		expect(screen.getByRole('group').all(), 'the pictures inside have no role').toHaveLength(1);
		await screen.getByRole('button', { name: 'Board 5, play here' }).click();
		expect(onstep.mock.calls).toEqual([[[4]]]);
	});

	it('draws the board grammar at both levels', async () => {
		const view = position((v) => {
			at(v, [0]).status = { Won: 'X' };
			at(v, [0, 4]).status = { Won: 'X' };
			at(v, [8]).status = 'Drawn';
			at(v, [4]).active = true;
			at(v, [4, 7]).active = true;
			v.sent = [4, 7];
			v.cells[indexOf([4, 3, 7])] = 'O';
			v.last = indexOf([4, 3, 7]);
		}, 3);
		const screen = await render(Board, { view, onstep: () => {}, flash: { board: [2], tick: 1 } });
		await expect
			.element(screen.getByRole('button', { name: 'Board 1, won by X' }))
			.toHaveAttribute('data-status', 'won-x');
		expect(tile(screen, '0').querySelector('.boards')?.classList).toContain('faded');
		expect(tile(screen, '04').dataset.status).toBe('won-x');
		expect(tile(screen, '8').querySelector('.hatch')).not.toBeNull();
		expect(tile(screen, '4').dataset.active).toBe('true');
		expect(tile(screen, '47').dataset.active).toBe('true');
		expect(tile(screen, '47').dataset.sent).toBe('true');
		expect(tile(screen, '3').dataset.active).toBeUndefined();
		expect(screen.container.textContent, 'no room for the label').not.toContain('Sent here');
		expect(tile(screen, '43').querySelector('[data-last]')).not.toBeNull();
		expect(tile(screen, '2').dataset.flash).toBe('true');
	});

	it('watermarks a won middle Board that still takes moves under Continuation', async () => {
		const view = position((v) => {
			v.continuation = true;
			at(v, [6]).status = { Won: 'O' };
			at(v, [6]).active = true;
		}, 3);
		const screen = await render(Board, { view, onstep: () => {} });
		expect(tile(screen, '6').querySelector('[data-owner="O"]')?.classList).toContain('watermark');
		expect(tile(screen, '6').querySelector('.boards')?.classList).not.toContain('faded');
	});
});
