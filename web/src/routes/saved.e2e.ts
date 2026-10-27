import { expect, test, type Page } from '@playwright/test';

/** Engine-test digit pairs as Notation: `"42"` (Space 4, then Space 2) is Cell 5·3. */
const notation = (digits: string) => [...digits].map((digit) => Number(digit) + 1).join('·');

/** X takes Local boards 0, 1 and 2, each with a row, while O sends X wherever it needs to go. */
const TOP_ROW = '03 30 04 40 05 51 13 31 14 41 15 52 26 62 27 72 28';

const cell = (page: Page, name: string) =>
	page.getByRole('button', { name: `Cell ${name}`, exact: true });

const play = async (page: Page, moves: string) => {
	for (const digits of moves.split(/\s+/)) await cell(page, notation(digits)).click();
};

const marked = (page: Page) => page.getByRole('button', { name: /^Cell \S+, [XO]$/ });

test('a reload resumes a game against the computer where it stood', async ({ page }) => {
	await page.goto('/play?vs=computer&level=1&seat=X');
	await expect(page).toHaveURL(/\/play\?rules=std&vs=computer&level=1&seat=X$/);
	await cell(page, '5·5').click();
	await expect(page).toHaveURL(/\/play\?game=[\w-]+$/);
	await expect(marked(page)).toHaveCount(2);
	const url = page.url();

	await page.reload();
	await expect(page).toHaveURL(url);
	await expect(marked(page)).toHaveCount(2);
	await expect(page.getByRole('button', { name: 'Cell 5·5, X' })).toBeVisible();
	await expect(page.getByRole('banner').getByText('vs Computer · Level 1')).toBeVisible();
	await page.locator('[aria-disabled="false"]').first().click();
	await expect(marked(page), 'the computer still answers').toHaveCount(4);
});

test('a finished game reopens at its final position', async ({ page }) => {
	await page.goto('/play');
	await play(page, TOP_ROW);
	await expect(page.getByRole('heading', { name: 'X wins on a line' })).toBeVisible();
	await page.reload();
	await expect(page.getByRole('heading', { name: 'X wins on a line' })).toBeVisible();
	await expect(marked(page)).toHaveCount(17);
	await expect(page.getByRole('button', { name: 'Undo' })).toBeDisabled();

	await page.getByRole('button', { name: 'New game' }).click();
	await expect(page, 'a new game until its first move').toHaveURL(/\/play\?rules=std$/);
});

test('an id this device doesn’t know says so', async ({ page }) => {
	await page.goto('/play?game=nowhere');
	await expect(page.getByText('This game isn’t saved on this device.')).toBeVisible();
	await expect(page.getByRole('link', { name: 'Choose a new game' })).toBeVisible();
});
