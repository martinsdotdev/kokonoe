import { expect, test, type Page } from '@playwright/test';
import { LINE_OF_MIDDLE_BOARDS, TOP_ROW } from '../lib/game/games.fixtures.js';
import { cell, focusBoard, marked, typeMoves } from './steps.js';

/** The game's one live line, which a screen reader reads after every change. */
const spoken = (page: Page) => page.locator('[data-spoken]');

test('a Standard game typed to a line win', async ({ page }) => {
	await page.goto('/play');
	await expect(spoken(page)).toHaveText('X: play anywhere.');
	await focusBoard(page);
	await typeMoves(page, TOP_ROW);
	await expect(page.getByRole('heading', { name: 'X wins on a line' })).toBeVisible();
	await expect(spoken(page)).toHaveText('X played 3·9. X wins on a line.');
});

test('a Deep game typed to a line of middle Boards, the view following', async ({ page }) => {
	await page.goto('/play?rules=d3');
	await expect(page.locator('[data-focus="global"]')).toBeVisible();
	await focusBoard(page);
	await typeMoves(page, LINE_OF_MIDDLE_BOARDS);
	await expect(page.getByRole('heading', { name: 'X wins on a line' })).toBeVisible();
	await expect(spoken(page)).toHaveText('X played 2·5·2. X wins on a line.');
});

test('digits type only on the board, as single-key shortcuts must (WCAG 2.1.4)', async ({
	page
}) => {
	await page.goto('/play');
	await page.getByRole('link', { name: 'Leave game' }).focus();
	await typeMoves(page, '44');
	await expect(marked(page)).toHaveCount(0);
});

test('the turn card shows a Cell being typed, until Escape or a move clears it', async ({
	page
}) => {
	await page.goto('/play');
	await focusBoard(page);
	await page.keyboard.press('5');
	await expect(page.getByText('Typing 5·_')).toBeVisible();
	await page.keyboard.press('Escape');
	await expect(page.getByText(/^Typing/)).toHaveCount(0);
	await page.keyboard.press('5');
	await cell(page, '44').click();
	await expect(page.getByText(/^Typing/), 'a move clears what was typed').toHaveCount(0);
});

test('arrows cross from Board to Board, Enter plays, and a typed refusal is said', async ({
	page
}) => {
	await page.goto('/play');
	await cell(page, '02').focus();
	await page.keyboard.press('ArrowRight');
	await expect(cell(page, '10'), 'into the next Board').toBeFocused();
	await page.keyboard.press('ArrowDown');
	await expect(cell(page, '13')).toBeFocused();
	await page.keyboard.press('Enter');
	await expect(spoken(page)).toHaveText('X played 2·4. O: play in board 4.');

	await typeMoves(page, '00');
	await expect(spoken(page)).toHaveText('X played 2·4. 1·1 isn’t playable. O: play in board 4.');
});

test('arrows move between the Boards of a stepped view', async ({ page }) => {
	await page.goto('/play?rules=d3');
	await page.getByRole('button', { name: /^Board 5\b/ }).focus();
	await page.keyboard.press('ArrowRight');
	await expect(page.getByRole('button', { name: /^Board 6\b/ })).toBeFocused();
	await page.keyboard.press('ArrowUp');
	await expect(page.getByRole('button', { name: /^Board 3\b/ })).toBeFocused();
});

test('a resignation is said without the last move again', async ({ page }) => {
	await page.goto('/play');
	await cell(page, '44').click();
	await page.getByRole('button', { name: 'Resign' }).click();
	await page.getByRole('button', { name: 'Yes, resign' }).click();
	await expect(spoken(page)).toHaveText('O resigned. X wins.');
});
