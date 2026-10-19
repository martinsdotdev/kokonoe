import { expect, test, type Page } from '@playwright/test';

const openHome = async (page: Page) => {
	await page.goto('/');
	await expect(page.getByRole('heading', { level: 1 })).toBeVisible();
};

const bodyBackgroundMatches = (page: Page, hex: string) =>
	page.evaluate((hex) => {
		const ctx = document.createElement('canvas').getContext('2d', { willReadFrequently: true })!;
		const rgb = (color: string) => {
			ctx.clearRect(0, 0, 1, 1);
			ctx.fillStyle = color;
			ctx.fillRect(0, 0, 1, 1);
			return [...ctx.getImageData(0, 0, 1, 1).data.slice(0, 3)];
		};
		const actual = rgb(getComputedStyle(document.body).backgroundColor);
		return rgb(hex).every((channel, i) => Math.abs(channel - actual[i]) <= 1);
	}, hex);

test('a deep link loads the app through the fallback page', async ({ page }) => {
	const response = await page.goto('/r/abc');
	expect(response?.status()).toBe(200);
	await expect(page).toHaveTitle('Kokonoe');
});

test('the heading is set in self-hosted Fraunces', async ({ page }) => {
	await openHome(page);
	const loaded = await page.evaluate(async () => {
		await document.fonts.ready;
		return document.fonts.check('600 48px "Fraunces Variable"');
	});
	expect(loaded).toBe(true);
	await expect(page.getByRole('heading', { level: 1 })).toHaveCSS(
		'font-family',
		/Fraunces Variable/
	);
});

test.describe('light', () => {
	test.use({ colorScheme: 'light' });

	test('the page is paper', async ({ page }) => {
		await openHome(page);
		await expect(page.locator('html')).not.toHaveClass(/dark/);
		expect(await bodyBackgroundMatches(page, '#F6F1E7')).toBe(true);
	});
});

test.describe('dark', () => {
	test.use({ colorScheme: 'dark' });

	test('the page is the night notebook', async ({ page }) => {
		await openHome(page);
		await expect(page.locator('html')).toHaveClass(/dark/);
		expect(await bodyBackgroundMatches(page, '#1C1D21')).toBe(true);
	});
});
