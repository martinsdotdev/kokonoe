import { expect, it, vi } from 'vitest';
import { render } from 'vitest-browser-svelte';
import GameControls from './GameControls.svelte';
import type { Controls } from './screen.js';

const action = (enabled: boolean) => ({ enabled, run: vi.fn() });

it('resigns only once confirmed, hiding the other controls meanwhile', async () => {
	const controls: Controls = { resigner: 'X', resign: vi.fn(), offerDraw: action(true) };
	const screen = await render(GameControls, { controls });
	await screen.getByRole('button', { name: 'Resign' }).click();
	await expect.element(screen.getByText('Resign as X?')).toBeVisible();
	expect(screen.getByRole('button', { name: 'Offer draw' }).all()).toHaveLength(0);
	await screen.getByRole('button', { name: 'No' }).click();
	expect(controls.resign).not.toHaveBeenCalled();
	await screen.getByRole('button', { name: 'Resign' }).click();
	await screen.getByRole('button', { name: 'Yes, resign' }).click();
	expect(controls.resign).toHaveBeenCalledOnce();
});

it('drops the question once someone else would resign, or no one can', async () => {
	const resign = vi.fn();
	const screen = await render(GameControls, { controls: { resigner: 'X', resign } });
	await screen.getByRole('button', { name: 'Resign' }).click();
	await screen.rerender({ controls: { resigner: null, resign } });
	await screen.rerender({ controls: { resigner: 'X', resign } });
	await expect.element(screen.getByRole('button', { name: 'Resign' })).toBeVisible();
	expect(screen.getByText('Resign as X?').all()).toHaveLength(0);
});

it('shows the controls the game has, each as it allows', async () => {
	const controls: Controls = {
		resigner: 'O',
		resign: vi.fn(),
		offerDraw: action(true),
		askTakeback: action(false)
	};
	const screen = await render(GameControls, { controls });
	expect(screen.getByRole('button', { name: 'Undo' }).all()).toHaveLength(0);
	await screen.getByRole('button', { name: 'Offer draw' }).click();
	expect(controls.offerDraw?.run).toHaveBeenCalledOnce();
	const takeback = screen.getByRole('button', { name: 'Ask takeback' });
	await expect.element(takeback).toHaveAttribute('aria-disabled', 'true');
	await takeback.click({ force: true });
	expect(
		controls.askTakeback?.run,
		'a control that can’t be used does nothing'
	).not.toHaveBeenCalled();
});

it('undoes on this device', async () => {
	const controls: Controls = { resigner: 'X', resign: vi.fn(), undo: action(true) };
	const screen = await render(GameControls, { controls });
	await screen.getByRole('button', { name: 'Undo' }).click();
	expect(controls.undo?.run).toHaveBeenCalledOnce();
});

it('has nothing to resign once the game is over', async () => {
	const controls: Controls = { resigner: null, resign: vi.fn(), undo: action(false) };
	const screen = await render(GameControls, { controls });
	const resign = screen.getByRole('button', { name: 'Resign' });
	await expect.element(resign).toHaveAttribute('aria-disabled', 'true');
	await expect
		.element(screen.getByRole('button', { name: 'Undo' }))
		.toHaveAttribute('aria-disabled', 'true');
	await resign.click({ force: true });
	expect(screen.getByText(/^Resign as/).all()).toHaveLength(0);
});
