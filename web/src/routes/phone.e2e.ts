import { expect, test, type Locator } from '@playwright/test';
import {
	DRAWN,
	INTO_A_WON_BOARD,
	LINE_OF_MIDDLE_BOARDS,
	TOP_ROW
} from '../lib/game/games.fixtures.js';
import { settledFocus, stepInto, stepThrough, stepped, tally } from './steps.js';

// A phone, set by hand: Playwright's iPhone descriptors bring WebKit and a shorter viewport.
test.use({ viewport: { width: 390, height: 844 }, hasTouch: true, isMobile: true });

const tap = (target: Locator) => target.tap();

test('a whole game plays with taps, one per Sent move', async ({ page }) => {
	await page.goto('/play');
	const taps = await stepThrough(page, TOP_ROW, tap);
	await expect(page.getByRole('heading', { name: 'X wins on a line' })).toBeVisible();
	await expect(stepped(page), 'once the game is over').toHaveAttribute('data-focus', 'global');
	expect(taps, 'the first move steps in').toEqual([2, ...Array(16).fill(1)]);
});

test('a drawn game follows every Free move out to the Global view', async ({ page }) => {
	await page.goto('/play');
	const taps = await stepThrough(page, DRAWN, tap);
	await expect(page.getByRole('heading', { name: 'Draw' })).toBeVisible();
	await expect(stepped(page)).toHaveAttribute('data-focus', 'global');
	expect(tally(taps)[2], 'the game has Free moves past the first').toBeGreaterThan(1);
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
	await expect(out.locator('div[aria-hidden="true"]'), 'a picture of the board').toHaveCount(1);
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
		await stepThrough(page, TOP_ROW, tap);
		await expect(page.getByRole('heading', { name: 'X wins on a line' })).toBeVisible();
	});
});

test.describe('Deep', () => {
	test('a whole game plays a level a tap: one tap Sent, two into a middle Board, three anywhere', async ({
		page
	}) => {
		test.slow();
		await page.goto('/play?rules=d3');
		const taps = await stepThrough(page, LINE_OF_MIDDLE_BOARDS, tap);
		await expect(page.getByRole('heading', { name: 'X wins on a line' })).toBeVisible();
		await expect(stepped(page)).toHaveAttribute('data-focus', 'global');
		// The first move and two Free moves anywhere take three; five Free moves in a middle Board two.
		expect(tally(taps)).toEqual({ 1: 45, 2: 5, 3: 3 });
		await expect(page.getByText('board·board·cell')).toBeVisible();
	});

	test('highlights where to play at every level after a Free move inside a middle Board', async ({
		page
	}) => {
		await page.goto('/play?rules=d3');
		await stepThrough(page, INTO_A_WON_BOARD, tap);
		// O Sent X into Local board 8·7, which X has won: X may play anywhere in Board 8.
		expect(await settledFocus(page)).toEqual([7]);
		await expect(page.getByText('X: play anywhere in board 8.')).toBeVisible();
		const won = page.locator('[data-board="76"]');
		await expect(won).toHaveAttribute('data-status', 'won-x');
		await expect(won).toHaveAttribute('data-sent');
		await expect(won).not.toHaveAttribute('data-active');
		await expect(page.locator('button[data-board^="7"][data-active]')).toHaveCount(8);

		await page.getByRole('button', { name: 'Global', exact: true }).tap();
		expect(await settledFocus(page)).toEqual([]);
		await expect(page.locator('button[data-board][data-active]')).toHaveCount(1);
		await expect(page.locator('button[data-board="7"]')).toHaveAttribute('data-active');

		await stepInto(page, [7]).tap();
		await settledFocus(page);
		await stepInto(page, [7, 0]).tap();
		expect(await settledFocus(page)).toEqual([7, 0]);
		await expect(page.locator('[data-board="70"]')).toHaveAttribute('data-active');
		await expect(page.getByRole('button', { name: 'Cell 8·1·1', exact: true })).toHaveAttribute(
			'aria-disabled',
			'false'
		);
	});

	test('the computer’s move lands in its Local board before the view follows', async ({ page }) => {
		await page.goto('/play?rules=d3&vs=computer&level=1&seat=X');
		await stepThrough(page, '444', tap);
		// X's 5·5·5 Sends the computer to Local board 5·5, still in view, where its move lands.
		const reply = page.getByRole('button', { name: /^Cell 5·5·\d, O$/ });
		await expect(reply).toBeVisible();
		await expect(stepped(page)).toHaveAttribute('data-focus', '44');
		const [, to] = (await reply.getAttribute('aria-label'))!.match(/^Cell 5·5·(\d)/)!;
		await expect(stepped(page)).toHaveAttribute('data-focus', `4${Number(to) - 1}`);
	});
});
