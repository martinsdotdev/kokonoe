import { expect, it, vi } from 'vitest';
import { render } from 'vitest-browser-svelte';
import ClaimCard from './ClaimCard.svelte';

const claim = (enabled = true) => ({ other: 'guest-1066', due: 0, enabled, run: vi.fn() });

it('offers the win or a draw against the player who left', async () => {
	const due = claim();
	const screen = await render(ClaimCard, { claim: due });
	await expect
		.element(screen.getByRole('region', { name: 'Claim' }))
		.toHaveTextContent(
			'guest-1066 left the game. You can claim the win or a draw, or keep waiting.'
		);
	await screen.getByRole('button', { name: 'Claim the win' }).click();
	await screen.getByRole('button', { name: 'Claim a draw' }).click();
	expect(due.run.mock.calls).toEqual([['Win'], ['Draw']]);
});

it('keeps its buttons, focusable, while a Claim can’t be made', async () => {
	const waiting = claim(false);
	const screen = await render(ClaimCard, { claim: waiting });
	const win = screen.getByRole('button', { name: 'Claim the win' });
	await expect.element(win).toHaveAttribute('aria-disabled', 'true');
	await win.click({ force: true });
	expect(waiting.run).not.toHaveBeenCalled();
	await expect.element(win).toHaveFocus();
});
