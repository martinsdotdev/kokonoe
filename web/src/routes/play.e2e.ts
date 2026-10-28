import { expect, test, type Page } from '@playwright/test';
import { DRAWN, TOP_ROW, notation } from './games.js';

const cell = (page: Page, digits: string) =>
	page.getByRole('button', { name: `Cell ${notation(digits)}`, exact: true });

const play = async (page: Page, moves: string) => {
	for (const digits of moves.split(/\s+/).filter(Boolean)) await cell(page, digits).click();
};

const marked = (page: Page) => page.getByRole('button', { name: /^Cell \S+, [XO]$/ });

const open = async (page: Page) => {
	await page.goto('/play');
	await expect(page.getByRole('group', { name: 'The Global board' })).toBeVisible();
};

test('the game screen loads the engine in the built app', async ({ page }) => {
	const wasm = page.waitForResponse((response) => response.url().endsWith('.wasm'));
	await open(page);
	await expect(page.getByText('X: play anywhere.')).toBeVisible();
	// Anything else makes the browser compile from a buffered copy, with a console warning.
	expect((await wasm).headers()['content-type']).toBe('application/wasm');
});

test('the engine still loads from a host that sends the wrong type', async ({ page }) => {
	await page.route('**/*.wasm', async (route) => {
		const response = await route.fetch();
		await route.fulfill({ response, contentType: 'application/octet-stream' });
	});
	await open(page);
	await expect(page.getByText('X: play anywhere.')).toBeVisible();
});

test('a Standard game plays to a line win, and then stays finished', async ({ page }) => {
	await open(page);
	await play(page, TOP_ROW);
	await expect(page.getByRole('heading', { name: 'X wins on a line' })).toBeVisible();
	await expect(page.getByRole('button', { name: 'Undo' })).toBeDisabled();
	await expect(page.getByRole('button', { name: 'Resign' })).toBeDisabled();
	await expect(marked(page)).toHaveCount(17);

	await page.getByRole('button', { name: 'New game' }).click();
	await expect(marked(page)).toHaveCount(0);
	await expect(page.getByText('X: play anywhere.')).toBeVisible();
});

test('a Standard game plays to a draw', async ({ page }) => {
	await open(page);
	await play(page, DRAWN);
	await expect(page.getByRole('heading', { name: 'Draw' })).toBeVisible();
	await expect(marked(page)).toHaveCount(43);
});

test('undo walks back to the first move', async ({ page }) => {
	await open(page);
	await play(page, '42 24 40 03 31');
	await expect(marked(page)).toHaveCount(5);
	const undo = page.getByRole('button', { name: 'Undo' });
	for (let i = 0; i < 5; i++) await undo.click();
	await expect(marked(page)).toHaveCount(0);
	await expect(undo).toBeDisabled();
	await expect(page.getByText('X: play anywhere.')).toBeVisible();
});

test('an illegal tap flashes the Board it names and plays nothing', async ({ page }) => {
	await open(page);
	await play(page, '42');
	// aria-disabled Cells still take clicks in a browser; Playwright needs telling.
	await cell(page, '00').click({ force: true });
	await expect(page.getByRole('group', { name: 'Board 3', exact: true })).toHaveAttribute(
		'data-flash'
	);
	await expect(marked(page)).toHaveCount(1);
});

test('resigning asks first, then ends the game', async ({ page }) => {
	await open(page);
	await play(page, '42');
	await page.getByRole('button', { name: 'Resign' }).click();
	await expect(page.getByText('Resign as O?')).toBeVisible();
	await page.getByRole('button', { name: 'Yes, resign' }).click();
	await expect(page.getByRole('heading', { name: 'O resigned. X wins.' })).toBeVisible();
	await expect(page.getByRole('button', { name: 'Undo' })).toBeDisabled();
});

test('a link naming no variant says so, and points to a new game', async ({ page }) => {
	await page.goto('/play?rules=nonsense');
	await expect(page.getByText('Kokonoe doesn’t know the variant “nonsense”')).toBeVisible();
	await page.getByRole('link', { name: 'Choose a new game' }).click();
	await expect(page).toHaveURL(/\/new-game$/);
});

test('any Ruleset code plays, as a Custom variant when it names no Preset', async ({ page }) => {
	await page.goto('/play?rules=both+first3');
	await expect(page.locator('[data-variant]')).toHaveText('Custom variant');
	await expect(page.locator('[data-score]')).toHaveText('X 0 · O 0 · first to 3');
	await cell(page, '42').click();
	await expect(marked(page)).toHaveCount(1);
});

test('a Ruleset code in the wrong order says why', async ({ page }) => {
	await page.goto('/play?rules=most+cont');
	await expect(
		page.getByText(
			'“most+cont”: the settings of a Ruleset code go in the order d3, cont, both, first, most, rand.'
		)
	).toBeVisible();
});
