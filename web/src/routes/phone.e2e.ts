import { expect, test, type Page } from '@playwright/test';

// A phone, set by hand: Playwright's iPhone descriptors bring WebKit and a shorter viewport.
test.use({ viewport: { width: 390, height: 844 }, hasTouch: true, isMobile: true });

/** X takes Local boards 0, 1 and 2, each with a row, while O sends X wherever it needs to go. */
const TOP_ROW = '03 30 04 40 05 51 13 31 14 41 15 52 26 62 27 72 28';

/** The shortest drawn game among 200,000 random ones, from the engine's tests. */
const DRAWN =
	'82 21 12 27 72 24 46 64 42 78 85 54 44 11 16 68 87 70 03 32 65 56 ' +
	'62 01 14 67 77 71 38 83 36 63 34 07 76 66 84 04 37 74 80 52 81';

/** Engine-test digit pairs, as Spaces: `"42"` is Cell 5·3, in Local board Space 4. */
const parse = (moves: string) =>
	moves.split(/\s+/).map((digits) => ({ space: Number(digits[0]), inner: Number(digits[1]) }));

const stepped = (page: Page) => page.locator('[data-focus]');

/** Waits for the view to stop stepping, and reports what it shows. */
const settledFocus = async (page: Page) => {
	await expect(stepped(page)).toHaveAttribute('data-settled');
	return stepped(page).getAttribute('data-focus');
};

/**
 * Plays `moves` with taps only, stepping out and in only when the view shows another Board, and
 * checks after each move that the view followed: to the Sent target, or the Global view on a
 * Free move and once the game is over. Returns how many taps it took.
 */
const tapThrough = async (page: Page, moves: string) => {
	const played = parse(moves);
	let taps = 0;
	for (const [i, { space, inner }] of played.entries()) {
		let focus = await settledFocus(page);
		if (focus !== 'global' && focus !== String(space)) {
			await page.getByRole('button', { name: 'Global', exact: true }).tap();
			taps++;
			focus = await settledFocus(page);
		}
		if (focus === 'global') {
			await page.getByRole('button', { name: new RegExp(`^Board ${space + 1}\\b`) }).tap();
			taps++;
			await settledFocus(page);
		}
		await page.getByRole('button', { name: `Cell ${space + 1}·${inner + 1}`, exact: true }).tap();
		taps++;
		const next = played[i + 1];
		const sent = next?.space === inner ? String(inner) : 'global';
		await expect(stepped(page), `after move ${i + 1}`).toHaveAttribute('data-focus', sent);
	}
	return taps;
};

/** Free moves in `moves`, the first move among them: each takes a tap on a Board first. */
const free = (moves: string) =>
	parse(moves).filter((move, i, all) => i === 0 || move.space !== all[i - 1].inner).length;

test('a whole game plays with taps, one per Sent move', async ({ page }) => {
	await page.goto('/play');
	const taps = await tapThrough(page, TOP_ROW);
	await expect(page.getByRole('heading', { name: 'X wins on a line' })).toBeVisible();
	expect(taps).toBe(parse(TOP_ROW).length + free(TOP_ROW));
});

test('a drawn game follows every Free move out to the Global view', async ({ page }) => {
	await page.goto('/play');
	const taps = await tapThrough(page, DRAWN);
	await expect(page.getByRole('heading', { name: 'Draw' })).toBeVisible();
	expect(free(DRAWN), 'the game has Free moves past the first').toBeGreaterThan(1);
	expect(taps).toBe(parse(DRAWN).length + free(DRAWN));
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

test('the computer’s move lands in view before the view follows', async ({ page }) => {
	await page.goto('/play?vs=computer&level=1&seat=X');
	await page.getByRole('button', { name: 'Board 5, play here' }).tap();
	await settledFocus(page);
	await page.getByRole('button', { name: 'Cell 5·5', exact: true }).tap();
	// The computer is Sent to Board 5, so its move lands in the Board in view.
	const reply = page.getByRole('button', { name: /^Cell 5·\d, O$/ });
	await expect(reply).toBeVisible();
	await expect(stepped(page)).toHaveAttribute('data-focus', '4');
	await expect(page.getByText(/^O’s move 5·\d Sent X there\.$/)).toBeVisible();
	const name = (await reply.getAttribute('aria-label'))!;
	const target = Number(name.match(/5·(\d)/)![1]) - 1;
	await expect(stepped(page)).toHaveAttribute('data-focus', String(target));
});

test.describe('with reduced motion', () => {
	test.use({ reducedMotion: 'reduce' });

	test('steps without animating, and still follows the game', async ({ page }) => {
		await page.goto('/play');
		const taps = await tapThrough(page, TOP_ROW);
		await expect(page.getByRole('heading', { name: 'X wins on a line' })).toBeVisible();
		expect(taps).toBe(parse(TOP_ROW).length + free(TOP_ROW));
	});
});
