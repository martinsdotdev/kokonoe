import { describe, expect, it, vi } from 'vitest';
import { render } from 'vitest-browser-svelte';
import type { Player } from '#lib/engine.js';
import Board from './Board.svelte';
import type { BoardView } from './view.js';
import { position } from './view.fixtures.js';

/** Board 3 (Space 2), won by `player` along its top row. */
const winTopRight = (view: BoardView, player: Player) => {
	for (const inner of [0, 1, 2]) view.cells[2 * 9 + inner] = player;
	view.locals[2].status = { Won: player };
};

const local = (screen: Awaited<ReturnType<typeof render>>, label: string) =>
	screen.getByRole('group', { name: label, exact: true });

describe('the board grammar', () => {
	it('washes the Active boards', async () => {
		const screen = await render(Board, { view: position((v) => (v.locals[5].active = true)) });
		await expect.element(local(screen, 'Board 6')).toHaveAttribute('data-active');
		await expect.element(local(screen, 'Board 5')).not.toHaveAttribute('data-active');
	});

	it('outlines the Sent target and labels it in Notation', async () => {
		const screen = await render(Board, {
			view: position((v) => {
				v.sent = 5;
				v.locals[5].active = true;
			})
		});
		await expect.element(local(screen, 'Board 6')).toHaveAttribute('data-sent');
		await expect.element(screen.getByText('Sent → 6')).toBeVisible();
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

	it('inks a won board and fades its Cells', async () => {
		const screen = await render(Board, { view: position((v) => winTopRight(v, 'X')) });
		const board = local(screen, 'Board 3, won by X');
		await expect.element(board).toHaveAttribute('data-status', 'won-x');
		expect(board.element().querySelector('[data-owner="X"]')?.classList).not.toContain('watermark');
		expect(board.element().querySelector('.cells')?.classList).toContain('faded');
	});

	it('cross-hatches a drawn board without fading it', async () => {
		const screen = await render(Board, { view: position((v) => (v.locals[7].status = 'Drawn')) });
		const board = local(screen, 'Board 8, drawn');
		await expect.element(board).toHaveAttribute('data-status', 'drawn');
		expect(board.element().querySelector('.hatch')).not.toBeNull();
		expect(board.element().querySelector('.cells')?.classList).not.toContain('faded');
	});

	it('watermarks a won board that still takes moves under Continuation', async () => {
		const screen = await render(Board, {
			view: position((v) => {
				v.continuation = true;
				winTopRight(v, 'O');
				v.locals[2].active = true;
			})
		});
		const board = local(screen, 'Board 3, won by O');
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
				v.sent = 2;
				v.free = true;
				for (const space of [0, 1, 3, 4, 5, 6, 7, 8]) v.locals[space].active = true;
			})
		});
		await expect.element(local(screen, 'Board 3, won by X')).toHaveAttribute('data-sent');
		await expect.element(local(screen, 'Board 3, won by X')).not.toHaveAttribute('data-active');
		for (const label of ['Board 1', 'Board 5', 'Board 9']) {
			await expect.element(local(screen, label)).toHaveAttribute('data-active');
		}
	});
});

describe('clicks', () => {
	it('reports the Cell clicked, legal or not', async () => {
		const onplay = vi.fn();
		const screen = await render(Board, {
			view: position((v) => (v.locals[4].active = true)),
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
		const screen = await render(Board, { view: position(), flash: { space: 4, tick: 1 } });
		await expect.element(local(screen, 'Board 5')).toHaveAttribute('data-flash');
		await expect.element(local(screen, 'Board 4')).not.toHaveAttribute('data-flash');
	});
});
