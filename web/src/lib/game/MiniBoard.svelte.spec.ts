import { expect, it } from 'vitest';
import { render } from 'vitest-browser-svelte';
import MiniBoard from './MiniBoard.svelte';
import { position } from './view.fixtures.js';
import { at } from './view.js';

it('draws a depth-2 board with its Cells, as one image', async () => {
	const view = position((v) => (v.cells[40] = 'X'));
	const screen = await render(MiniBoard, { view, label: 'The board after 1 move' });
	await expect.element(screen.getByRole('img', { name: 'The board after 1 move' })).toBeVisible();
	expect(screen.container.querySelectorAll('.cell')).toHaveLength(81);
	expect(screen.container.querySelectorAll('.glyph')).toHaveLength(1);
});

it('draws a whole Deep board by its Boards, and a middle Board with its Cells', async () => {
	const view = position((v) => {
		for (const cell of [0, 1, 2]) v.cells[cell] = 'X';
		at(v, [0, 0]).status = { Won: 'X' };
		at(v, [4]).active = true;
	}, 3);
	const whole = await render(MiniBoard, { view, label: 'Deep' });
	expect(whole.container.querySelectorAll('.tile')).toHaveLength(9 + 81);
	expect(whole.container.querySelectorAll('.cell'), 'too small to see').toHaveLength(0);
	expect(whole.container.querySelectorAll('.owner')).toHaveLength(1);
	expect(whole.container.querySelectorAll('.tile[data-active]')).toHaveLength(1);

	const middle = await render(MiniBoard, { view, board: [0], label: 'Board 1' });
	expect(middle.container.querySelectorAll('.cell')).toHaveLength(81);
	expect(middle.container.querySelectorAll('.glyph')).toHaveLength(3);
});

it('draws dealt marks in pencil grey', async () => {
	const view = position((v) => {
		v.cells[40] = 'X';
		v.cells[36] = 'O';
		v.dealt = [40];
	});
	const screen = await render(MiniBoard, { view });
	expect(screen.container.querySelectorAll('[stroke="var(--dealt)"]')).toHaveLength(1);
	expect(screen.container.querySelectorAll('[stroke="var(--o)"]')).toHaveLength(1);
});
