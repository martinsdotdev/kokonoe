import { expect, test } from '@playwright/test';
import { stepThrough } from './steps.js';

const TWO_LEVELS = ['Standard', 'Forced continuation', 'Tic-Tac-Ku', 'Count tiebreak'];

test('every Preset at depth 2 starts pass and play from Home', async ({ page }) => {
	for (const name of TWO_LEVELS) {
		await page.goto('/');
		await page.getByRole('link', { name: `Play ${name}, pass and play` }).click();
		await expect(page.locator('[data-variant]')).toHaveText(name);
		await expect(page.getByRole('banner').getByText('Pass and play')).toBeVisible();
		await page.getByRole('button', { name: 'Cell 5·5', exact: true }).click();
		await expect(page.getByRole('button', { name: 'Cell 5·5, X' })).toBeVisible();
	}
});

test('Deep starts from Home, one Board at a time', async ({ page }) => {
	await page.goto('/');
	await page.getByRole('link', { name: 'Play Deep, pass and play' }).click();
	await expect(page).toHaveURL(/\/play\?rules=d3$/);
	await expect(page.locator('[data-variant]')).toHaveText('Deep');
	expect(await stepThrough(page, '444', (target) => target.click())).toEqual([3]);
	await expect(page.getByRole('button', { name: 'Cell 5·5·5, X' })).toBeVisible();
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

test('Home shows the Guest’s Handle once the server starts a session', async ({ page }) => {
	const methods: string[] = [];
	await page.route('/api/session', (route) => {
		methods.push(route.request().method());
		return route.fulfill({ json: { handle: 'guest-4821' } });
	});
	await page.goto('/');
	await expect(page.locator('[data-account]')).toHaveText('Playing as guest-4821');
	expect(methods).toEqual(['POST']);
});

test('Home works without the server, showing no Account', async ({ page }) => {
	const errors: Error[] = [];
	page.on('pageerror', (error) => errors.push(error));
	const answered = page.waitForResponse('/api/session');
	await page.goto('/');
	// sirv answers with its fallback page, as any static host might.
	expect((await answered).headers()['content-type']).toContain('text/html');
	await page.waitForLoadState('networkidle');
	await expect(page.getByRole('link', { name: 'Import game' })).toBeVisible();
	await expect(page.locator('[data-account]')).toHaveCount(0);
	expect(errors).toEqual([]);
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
