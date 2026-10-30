import { expect, test, type Page } from '@playwright/test';
import { TOP_ROW } from '../lib/game/games.fixtures.js';
import { cell, marked, play, stepped, stepThrough, turnLine } from './steps.js';

/**
 * Waits until IndexedDB holds `moves` moves of the game in the address. A save is still in
 * flight just after a move, and a full page load would abort it. It runs in the page, so it
 * repeats `saved.ts`'s names: the `kokonoe` database and its `games` store.
 */
const waitForSaved = (page: Page, moves: number) =>
	page.waitForFunction(async (moves) => {
		const id = new URLSearchParams(location.search).get('game');
		if (!id) return false;
		const request = <T>(r: IDBRequest<T>) =>
			new Promise<T>((resolve, reject) => {
				r.onsuccess = () => resolve(r.result);
				r.onerror = () => reject(r.error);
			});
		const db = await request(indexedDB.open('kokonoe'));
		const game = await request(db.transaction('games').objectStore('games').get(id));
		db.close();
		return game?.record.moves.length === moves;
	}, moves);

test('a reload resumes a game against the computer where it stood', async ({ page }) => {
	await page.goto('/play?vs=computer&level=1&seat=X');
	await expect(page).toHaveURL(/\/play\?rules=std&vs=computer&level=1&seat=X$/);
	await cell(page, '44').click();
	await expect(page).toHaveURL(/\/play\?game=[\w-]+$/);
	await expect(marked(page)).toHaveCount(2);
	await waitForSaved(page, 2);
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
	await waitForSaved(page, 17);
	await page.reload();
	await expect(page.getByRole('heading', { name: 'X wins on a line' })).toBeVisible();
	await expect(marked(page)).toHaveCount(17);
	await expect(page.getByRole('button', { name: 'Undo' })).toBeDisabled();

	await page.getByRole('button', { name: 'New game' }).click();
	await expect(page, 'a new game until its first move').toHaveURL(/\/play\?rules=std$/);
});

test('a Deep game resumes in the Board it stood in, and Home draws it', async ({ page }) => {
	await page.goto('/play?rules=d3');
	await stepThrough(page, '444 440', (target) => target.click());
	await expect(page).toHaveURL(/\/play\?game=[\w-]+$/);
	await waitForSaved(page, 2);
	await page.reload();
	await expect(turnLine(page, 'X: play in board 5·1.')).toBeVisible();
	await expect(stepped(page)).toHaveAttribute('data-focus', '40');

	await page.getByRole('link', { name: 'Leave game' }).click();
	const card = page.getByRole('article', { name: 'Continue game' });
	await expect(card.getByText('Deep · 2 moves')).toBeVisible();
	await expect(card.getByRole('img', { name: 'The board after 2 moves' })).toBeVisible();
});

test('an id this device doesn’t know says so', async ({ page }) => {
	await page.goto('/play?game=nowhere');
	await expect(page.getByText('This game isn’t saved on this device.')).toBeVisible();
	await expect(page.getByRole('link', { name: 'Choose a new game' })).toBeVisible();
});

test('Home continues the latest unfinished game', async ({ page }) => {
	await page.goto('/play?vs=computer&level=1&seat=X');
	await cell(page, '44').click();
	await expect(marked(page)).toHaveCount(2);
	await page.getByRole('link', { name: 'Leave game' }).click();

	const card = page.getByRole('article', { name: 'Continue game' });
	await expect(card.getByText('Your move against the computer')).toBeVisible();
	await expect(card.getByText('Standard · Level 1 · 2 moves')).toBeVisible();
	await expect(card.getByRole('img', { name: 'The board after 2 moves' })).toBeVisible();
	await card.getByRole('link', { name: 'Resume' }).click();
	await expect(page).toHaveURL(/\/play\?game=[\w-]+$/);
	await expect(marked(page)).toHaveCount(2);
});

test('Recent games lists finished and unfinished games, and opens finished ones to replay', async ({
	page
}) => {
	await page.goto('/play');
	await play(page, TOP_ROW);
	await waitForSaved(page, 17);
	for (const code of ['most', 'cont']) {
		await page.goto(`/play?rules=${code}`);
		await play(page, '42');
		await waitForSaved(page, 1);
	}
	await page.goto('/');

	const rows = page.locator('[data-recent]');
	await expect(rows).toHaveCount(3);
	await expect(rows.nth(0)).toContainText('Forced continuation');
	await expect(rows.nth(1), 'the older unfinished game stays').toContainText('Count tiebreak');
	await expect(page.locator('[data-recent="unfinished"]')).toHaveCount(2);
	await expect(page.locator('[data-recent="unfinished"]').first()).toContainText(
		'Unfinished · Resume'
	);
	const card = page.getByRole('article', { name: 'Continue game' });
	await expect(card).toContainText('O to move · pass and play');
	await expect(card).toContainText('Forced continuation · 1 move');
	const finished = page.locator('[data-recent="finished"]');
	await expect(finished).toContainText('X wins on a line · Replay');
	await finished.click();
	await expect(page).toHaveURL(/\/replay\?game=[\w-]+$/);
	await expect(page.getByRole('heading', { name: 'X wins on a line' })).toBeVisible();
	// A replay's Cells are images to read, not buttons.
	const shown = page.getByRole('img', { name: /^Cell \S+, [XO]$/ });
	await expect(shown).toHaveCount(0);
	await page.getByRole('button', { name: 'Last move' }).click();
	await expect(shown).toHaveCount(17);
});

test('a saved game the rules refuse says why it can’t be resumed', async ({ page }) => {
	await page.goto('/play');
	await play(page, '42');
	await waitForSaved(page, 1);
	await page.evaluate(async () => {
		const id = new URLSearchParams(location.search).get('game')!;
		const db = await new Promise<IDBDatabase>((resolve) => {
			indexedDB.open('kokonoe').onsuccess = (event) =>
				resolve((event.target as IDBOpenDBRequest).result);
		});
		const store = db.transaction('games', 'readwrite').objectStore('games');
		await new Promise((resolve) => {
			store.get(id).onsuccess = (event) => {
				const game = (event.target as IDBRequest).result;
				game.record.moves = [0, 0];
				store.put(game).onsuccess = resolve;
			};
		});
		db.close();
	});
	await page.reload();
	await expect(page.getByText('This game can’t be resumed. Its move 2 isn’t legal.')).toBeVisible();
	await expect(page.getByRole('link', { name: 'Choose a new game' })).toBeVisible();
});
