import { expect, test } from '@playwright/test';

test('the sheet passes the variant, Level and seat to the game', async ({ page }) => {
	await page.goto('/new-game');
	await page.getByRole('radio', { name: /^Tic-Tac-Ku/ }).check();
	await page.getByLabel('Computer level').selectOption('4');
	await page.getByRole('radio', { name: 'O', exact: true }).check();
	await expect(
		page.getByRole('heading', { name: 'Tic-Tac-Ku against the computer' })
	).toBeVisible();
	await expect(page.getByText('O, moving second')).toBeVisible();
	await expect(page.getByText('First to 5 won boards')).toBeVisible();

	await page.getByRole('button', { name: 'Start game' }).click();
	await expect(page).toHaveURL(/\/play\?rules=cont%2Bfirst5%2Bmost&vs=computer&level=4&seat=O$/);
	await expect(page.getByRole('banner').getByText('vs Computer · Level 4')).toBeVisible();
	await expect(page.locator('[data-variant]')).toHaveText('Tic-Tac-Ku');
	await expect(
		page.getByRole('button', { name: /^Cell \S+, X$/ }),
		'the computer sits on X, and opens'
	).toHaveCount(1);
	await expect(page.locator('[data-score]')).toHaveText('X 0 · O 0 · first to 5');
	await expect(page.getByText('RULES · TIC-TAC-KU')).toBeVisible();
});

test('a Random seat lands on X or O', async ({ page }) => {
	await page.goto('/new-game?vs=computer');
	await page.getByRole('radio', { name: 'Random' }).check();
	await expect(page.getByText('X or O, drawn when you start')).toBeVisible();
	await page.getByRole('button', { name: 'Start game' }).click();
	await expect(page).toHaveURL(/&seat=[XO]$/);
});

test('pass and play needs no Level or seat', async ({ page }) => {
	await page.goto('/new-game?vs=hotseat&rules=most');
	await expect(page.getByLabel('Computer level')).toHaveCount(0);
	await expect(page.getByRole('radio', { name: /^Count tiebreak/ })).toBeChecked();
	await page.getByRole('button', { name: 'Start game' }).click();
	await expect(page).toHaveURL(/\/play\?rules=most$/);
	await expect(page.locator('[data-score]')).toHaveText('X 0 · O 0 · most boards wins');
});

test('Deep can’t be picked yet', async ({ page }) => {
	await page.goto('/new-game');
	await expect(page.getByRole('radio', { name: /^Deep/ })).toBeDisabled();
});
