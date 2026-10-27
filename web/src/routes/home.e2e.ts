import { expect, test } from '@playwright/test';

const PLAYABLE = ['Standard', 'Forced continuation', 'Tic-Tac-Ku', 'Count tiebreak'];

test('every playable Preset starts pass and play from Home', async ({ page }) => {
	for (const name of PLAYABLE) {
		await page.goto('/');
		await page.getByRole('link', { name: `Play ${name}, pass and play` }).click();
		await expect(page.locator('[data-variant]')).toHaveText(name);
		await expect(page.getByRole('banner').getByText('Pass and play')).toBeVisible();
		await page.getByRole('button', { name: 'Cell 5·5', exact: true }).click();
		await expect(page.getByRole('button', { name: 'Cell 5·5, X' })).toBeVisible();
	}
});

test('Deep is on the shelf but not yet playable', async ({ page }) => {
	await page.goto('/');
	const deep = page.getByRole('article', { name: 'Deep' });
	await expect(deep.getByText('ARRIVES SOON')).toBeVisible();
	await expect(deep.getByRole('link')).toHaveCount(0);
});

test('the computer card opens the sheet at the chosen Level', async ({ page }) => {
	await page.goto('/');
	await page.getByRole('button', { name: '4', exact: true }).click();
	await expect(page.getByText('Level 4 of 8')).toBeVisible();
	await page.getByRole('link', { name: 'Play the computer' }).click();
	await expect(page).toHaveURL(/\/new-game\?vs=computer&level=4$/);
	await expect(page.getByRole('radio', { name: /^Computer/ })).toBeChecked();
	await expect(page.getByLabel('Computer level')).toHaveValue('4');
});

test('Home loads no engine, with a saved game to continue', async ({ context, page }) => {
	await page.goto('/play');
	await page.getByRole('button', { name: 'Cell 5·5', exact: true }).click();
	await expect(page).toHaveURL(/\?game=/);

	// A second page shares the saved game, and loads Home from scratch.
	const home = await context.newPage();
	const loaded: string[] = [];
	const scripts: Promise<string>[] = [];
	home.on('response', (response) => {
		const path = new URL(response.url()).pathname;
		if (!/\.(m?js|wasm)$/.test(path)) return;
		loaded.push(path);
		scripts.push(response.text());
	});
	await home.goto('/');
	await expect(home.getByRole('link', { name: 'Resume', exact: true })).toBeVisible();
	await home.waitForLoadState('networkidle');
	expect(loaded.filter((path) => path.endsWith('.wasm'))).toEqual([]);
	// The engine's glue names its module, so no script Home loads may carry the name.
	for (const script of await Promise.all(scripts)) expect(script).not.toContain('kokonoe_wasm_bg');
	expect(scripts.length).toBeGreaterThan(0);
});
