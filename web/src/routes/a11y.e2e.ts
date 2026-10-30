import AxeBuilder from '@axe-core/playwright';
import { expect, test, type Page } from '@playwright/test';
import { INTO_A_WON_BOARD, TOP_ROW } from '../lib/game/games.fixtures.js';
import { focusBoard, play, typeMoves } from './steps.js';

/** WCAG 2.2 AA (spec §7.7), with the A and AA rules of every version before it. */
const TAGS = ['wcag2a', 'wcag2aa', 'wcag21a', 'wcag21aa', 'wcag22aa'];

/** X wins Local board 1, then O Sends X back into it: a Free move. */
const FREE_MOVE = '03 30 04 40 05 50';

/** Scans the page as it stands, and lists each rule it breaks with where and why. */
const scan = async (page: Page, what: string) => {
	const { violations } = await new AxeBuilder({ page }).withTags(TAGS).analyze();
	const found = violations.map((violation) =>
		[
			`${violation.id} (${violation.impact}): ${violation.help}`,
			...violation.nodes.slice(0, 5).map((node) => `  ${node.target.join(' ')}`),
			`  ${violation.helpUrl}`
		].join('\n')
	);
	expect(found, `${what} passes axe`).toEqual([]);
};

for (const colorScheme of ['light', 'dark'] as const) {
	test.describe(`in the ${colorScheme === 'light' ? 'paper' : 'night'} palette`, () => {
		test.use({ colorScheme });

		test('Home, New game and Import', async ({ page }) => {
			await page.goto('/');
			await expect(page.getByRole('link', { name: 'Import game' })).toBeVisible();
			await scan(page, 'Home');
			await page.goto('/new-game');
			await expect(page.getByRole('heading').first()).toBeVisible();
			await scan(page, 'New game');
			await page.goto('/import');
			await expect(page.getByRole('heading', { name: 'Import a game' })).toBeVisible();
			await scan(page, 'Import');
		});

		test('the game screen at depth 2, and the Replay viewer', async ({ page }) => {
			await page.goto('/play');
			await expect(page.getByRole('group', { name: 'The Global board' })).toBeVisible();
			await scan(page, 'the start');
			await play(page, FREE_MOVE);
			await expect(page.locator('[data-stamp]')).toBeVisible();
			await scan(page, 'a Free move');
			await page.goto('/play');
			await play(page, TOP_ROW);
			await expect(page.getByRole('heading', { name: 'X wins on a line' })).toBeVisible();
			await scan(page, 'the game over');
			await page.getByRole('link', { name: 'Replay' }).click();
			await expect(page.locator('[data-caption]')).toBeVisible();
			await scan(page, 'the Replay viewer');
		});

		test('the game screen in Deep', async ({ page }) => {
			await page.goto('/play?rules=d3');
			await expect(page.locator('[data-focus="global"]')).toBeVisible();
			await scan(page, 'the start');
			await focusBoard(page);
			await typeMoves(page, INTO_A_WON_BOARD);
			await expect(page.locator('[data-stamp]')).toBeVisible();
			await expect(page.locator('[data-settled]')).toBeVisible();
			await scan(page, 'a Free move inside a middle Board');
		});
	});
}
