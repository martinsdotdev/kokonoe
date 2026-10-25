import { expect, test } from '@playwright/test';

test('the game screen loads the engine in the built app', async ({ page }) => {
	const wasm = page.waitForResponse((response) => response.url().endsWith('.wasm'));
	await page.goto('/play');
	await expect(page.getByText('X to move')).toBeVisible();
	// Anything else makes the browser compile from a buffered copy, with a console warning.
	expect((await wasm).headers()['content-type']).toBe('application/wasm');
});
