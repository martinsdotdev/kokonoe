import { expect, test, type Page } from '@playwright/test';

const cell = (page: Page, notation: string) =>
	page.getByRole('button', { name: `Cell ${notation}`, exact: true });

const marked = (page: Page) => page.getByRole('button', { name: /^Cell \S+, [XO]$/ });

test('the computer answers the player, and undo takes back both moves', async ({ page }) => {
	const wasm: string[] = [];
	page.on('request', (request) => {
		if (request.url().endsWith('.wasm')) wasm.push(request.url());
	});
	await page.goto('/play?vs=computer&level=1&seat=X');
	await cell(page, '5·5').click();
	await expect(page.getByText('Level 1 · Thinking…')).toBeVisible();
	await expect(marked(page)).toHaveCount(2);
	await expect(page.getByText('Level 1 · Waiting')).toBeVisible();
	expect(wasm, 'the Worker gets the page’s compiled module').toHaveLength(1);
	await page.getByRole('button', { name: 'Undo' }).click();
	await expect(marked(page)).toHaveCount(0);
	await expect(page.getByRole('button', { name: 'Undo' })).toBeDisabled();
});

test('the computer opens when it sits on X', async ({ page }) => {
	await page.goto('/play?vs=computer&level=2&seat=O');
	await expect(page.getByRole('button', { name: /^Cell \S+, X$/ })).toHaveCount(1);
	await expect(page.getByRole('button', { name: 'Undo' })).toBeDisabled();
	await expect(page.getByText('You').locator('..')).toContainText('To move');
});

test('resigning against the computer resigns for the player', async ({ page }) => {
	await page.goto('/play?vs=computer&level=1&seat=X');
	await cell(page, '5·5').click();
	await page.getByRole('button', { name: 'Resign' }).click();
	await page.getByRole('button', { name: 'Yes, resign' }).click();
	await expect(page.getByRole('heading', { name: 'X resigned. O wins.' })).toBeVisible();
	await expect(marked(page), 'the search was stopped').toHaveCount(1);
});

test('leaving before the engine loads starts no computer', async ({ page }) => {
	let release = () => {};
	const held = new Promise<void>((resolve) => (release = resolve));
	await page.route('**/*.wasm', async (route) => {
		await held;
		await route.continue();
	});
	await page.goto('/new-game?vs=computer');
	await page.getByRole('radio', { name: 'O', exact: true }).check();
	await page.getByRole('button', { name: 'Start game' }).click();
	await expect(page.getByText('Loading the engine…')).toBeVisible();
	await page.goBack();
	await expect(page).toHaveURL(/\/new-game/);
	const loaded = page.waitForResponse((response) => response.url().endsWith('.wasm'));
	release();
	await loaded;
	// Long enough for the engine to load and a session to start its Worker.
	await page.waitForTimeout(500);
	expect(page.workers()).toHaveLength(0);
});

test('from Level 5 the computer searches across Workers, with no cross-origin isolation', async ({
	page
}) => {
	const response = await page.goto('/play?vs=computer&level=8&seat=O');
	const headers = response!.headers();
	expect(headers['cross-origin-opener-policy']).toBeUndefined();
	expect(headers['cross-origin-embedder-policy']).toBeUndefined();
	expect(await page.evaluate(() => crossOriginIsolated)).toBe(false);
	// Level 8 opens as X across min(4, cores − 1) Workers, at least one.
	await expect(page.getByRole('button', { name: /^Cell \S+, X$/ })).toHaveCount(1);
	const cores = await page.evaluate(() => navigator.hardwareConcurrency || 1);
	expect(page.workers()).toHaveLength(Math.min(4, Math.max(1, cores - 1)));
});
