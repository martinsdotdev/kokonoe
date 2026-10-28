import { expect, test, type Page } from '@playwright/test';
import { DRAWN, TOP_ROW, parse } from './games.js';

// A phone, set by hand: Playwright's iPhone descriptors bring WebKit and a shorter viewport.
test.use({ viewport: { width: 390, height: 844 }, hasTouch: true, isMobile: true });

const stepped = (page: Page) => page.locator('[data-focus]');

/** Waits for the view to stop stepping, and reports what it shows. */
const settledFocus = async (page: Page) => {
	await expect(stepped(page)).toHaveAttribute('data-settled');
	return stepped(page).getAttribute('data-focus');
};

/**
 * Plays `moves` with taps only, and checks before each move that the view followed: to the
 * move's Board when it was Sent there, or the Global view, where a tap on the Board steps in.
 * Returns how many taps it took.
 */
const tapThrough = async (page: Page, moves: string) => {
	const played = parse(moves);
	let taps = 0;
	for (const [i, { space, inner }] of played.entries()) {
		const sent = i > 0 && played[i - 1].inner === space;
		const focus = await settledFocus(page);
		expect(focus, `before move ${i + 1}`).toBe(sent ? String(space) : 'global');
		if (!sent) {
			await page.getByRole('button', { name: new RegExp(`^Board ${space + 1}\\b`) }).tap();
			taps++;
			await settledFocus(page);
		}
		await page.getByRole('button', { name: `Cell ${space + 1}·${inner + 1}`, exact: true }).tap();
		taps++;
	}
	await expect(stepped(page), 'once the game is over').toHaveAttribute('data-focus', 'global');
	return taps;
};

/** How many of `moves` start with a tap on a Board: the first move, and every Free move. */
const boardTaps = (moves: string) =>
	parse(moves).filter((move, i, all) => i === 0 || move.space !== all[i - 1].inner).length;

test('a whole game plays with taps, one per Sent move', async ({ page }) => {
	await page.goto('/play');
	const taps = await tapThrough(page, TOP_ROW);
	await expect(page.getByRole('heading', { name: 'X wins on a line' })).toBeVisible();
	expect(taps).toBe(parse(TOP_ROW).length + boardTaps(TOP_ROW));
});

test('a drawn game follows every Free move out to the Global view', async ({ page }) => {
	await page.goto('/play');
	const taps = await tapThrough(page, DRAWN);
	await expect(page.getByRole('heading', { name: 'Draw' })).toBeVisible();
	expect(boardTaps(DRAWN), 'the game has Free moves past the first').toBeGreaterThan(1);
	expect(taps).toBe(parse(DRAWN).length + boardTaps(DRAWN));
});

test('the board and the turn come first, above the fold', async ({ page }) => {
	for (const height of [844, 664]) {
		await page.setViewportSize({ width: 390, height });
		await page.goto('/play');
		await expect(page.getByRole('group', { name: 'The Global board' })).toBeInViewport({
			ratio: 1
		});
		await expect(page.getByText('X: play anywhere.')).toBeInViewport({ ratio: 1 });
	}
	const board = await page.getByRole('region', { name: 'Board' }).boundingBox();
	const players = await page.getByRole('complementary', { name: 'Players' }).boundingBox();
	expect(board!.y).toBeLessThan(players!.y);
});

test('the Global view takes Boards, and only the breadcrumb steps out', async ({ page }) => {
	await page.goto('/play');
	await expect(page.getByRole('button', { name: /^Cell / })).toHaveCount(0);
	await page.getByRole('button', { name: 'Board 5, play here' }).tap();
	await expect(stepped(page)).toHaveAttribute('data-focus', '4');
	await expect(page.getByRole('button', { name: /^Cell 5·/ })).toHaveCount(9);
	// One board at a time: no overview to tap beside it, only a picture inside the way out.
	await expect(page.getByRole('group', { name: 'The Global board' })).toHaveCount(0);
	const out = page.getByRole('button', { name: 'Global', exact: true });
	await expect(out.locator('[role="img"]')).toHaveCount(1);
	await expect(page.locator('[aria-current="location"]')).toHaveText('Board 5');
	await settledFocus(page);
	await out.tap();
	await expect(stepped(page)).toHaveAttribute('data-focus', 'global');
});

test('a refused tap steps to where to play, and says so until the next move', async ({ page }) => {
	await page.goto('/play');
	await page.getByRole('button', { name: 'Board 5, play here' }).tap();
	await settledFocus(page);
	await page.getByRole('button', { name: 'Cell 5·5', exact: true }).tap();
	await settledFocus(page);
	await page.getByRole('button', { name: 'Global', exact: true }).tap();
	await settledFocus(page);
	await page.getByRole('button', { name: 'Board 1' }).tap();
	await settledFocus(page);
	// aria-disabled Cells still take taps in a browser; Playwright needs telling.
	await page.getByRole('button', { name: 'Cell 1·1', exact: true }).tap({ force: true });
	await expect(stepped(page)).toHaveAttribute('data-focus', '4');
	const line = page.getByText('Not playable. O: play in board 5.');
	await expect(line).toBeVisible();
	await settledFocus(page);
	await page.getByRole('button', { name: 'Global', exact: true }).tap();
	// Past the flash: the view stays where it was put, and the line stays said.
	await page.waitForTimeout(1000);
	await expect(stepped(page)).toHaveAttribute('data-focus', 'global');
	await expect(line).toBeVisible();
	await page.getByRole('button', { name: 'Board 5, play here' }).tap();
	await settledFocus(page);
	await page.getByRole('button', { name: 'Cell 5·1', exact: true }).tap();
	await expect(page.getByText('X: play in board 1.')).toBeVisible();
});

test('the computer’s move lands in view before the view follows', async ({ page }) => {
	await page.goto('/play?vs=computer&level=1&seat=X');
	await page.getByRole('button', { name: 'Board 5, play here' }).tap();
	await settledFocus(page);
	await page.getByRole('button', { name: 'Cell 5·5', exact: true }).tap();
	// The computer is Sent to Board 5, so its move lands in the Board in view.
	// Without the hold the view would already be on the Board the reply Sends X to.
	const reply = page.getByRole('button', { name: /^Cell 5·\d, O$/ });
	await expect(reply).toBeVisible();
	await expect(stepped(page)).toHaveAttribute('data-focus', '4');
	await expect(page.getByText(/^O’s move 5·\d Sent X there\.$/)).toBeVisible();
	const [, to] = (await reply.getAttribute('aria-label'))!.match(/^Cell 5·(\d)/)!;
	await expect(stepped(page)).toHaveAttribute('data-focus', String(Number(to) - 1));
});

test.describe('with reduced motion', () => {
	test.use({ reducedMotion: 'reduce' });

	test('steps without animating, and still follows the game', async ({ page }) => {
		await page.goto('/play');
		const taps = await tapThrough(page, TOP_ROW);
		await expect(page.getByRole('heading', { name: 'X wins on a line' })).toBeVisible();
		expect(taps).toBe(parse(TOP_ROW).length + boardTaps(TOP_ROW));
	});
});
