import { page } from 'vitest/browser';
import { expect, it } from 'vitest';
import { render } from 'vitest-browser-svelte';
import Page from './+page.svelte';

it('names the game it plays', async () => {
	render(Page);
	await expect
		.element(page.getByRole('heading', { level: 1 }))
		.toHaveTextContent('Ultimate Tic-Tac-Toe');
});
