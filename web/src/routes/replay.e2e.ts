import { expect, test, type Page } from '@playwright/test';
import { DEALT, LINE_OF_MIDDLE_BOARDS, notation, parse } from '../lib/game/games.fixtures.js';
import { stepped, stepThrough } from './steps.js';

test.use({ viewport: { width: 1280, height: 720 }, reducedMotion: 'reduce' });

/** Deals every new game from `seed`, by answering the eight bytes `newSeed` asks for. */
const dealFrom = (page: Page, seed: string) =>
	page.addInitScript((hex) => {
		const random = crypto.getRandomValues.bind(crypto);
		crypto.getRandomValues = ((array: Uint8Array<ArrayBuffer>) => {
			if (array.length !== 8) return random(array);
			array.set(hex.match(/../g)!.map((byte) => Number.parseInt(byte, 16)));
			return array;
		}) as typeof crypto.getRandomValues;
	}, seed);

const caption = (page: Page) => page.locator('[data-caption]');

/** The Replay's address, without a ply. */
let address = '';

/** The address shows ply `n`. */
const ply = (page: Page, n: number) => expect(page).toHaveURL(n ? `${address}&ply=${n}` : address);

/** The move-list row for ply `n`. */
const row = (page: Page, n: number) =>
	page.getByRole('button', { name: new RegExp(`^${n} [XO] `) });

test('replays a finished Deep game with a Random opening, ply by ply', async ({ page }) => {
	test.slow();
	await dealFrom(page, DEALT.seed);
	await page.goto(`/play?rules=${DEALT.code}`);
	await expect(page.locator('[data-variant]')).toHaveText('Custom variant');
	const [x, o] = parse(DEALT.opening);
	await expect(page.getByText(`(${notation(x)})`)).toBeVisible();
	await expect(page.getByText(`(${notation(o)})`)).toBeVisible();
	await stepThrough(page, LINE_OF_MIDDLE_BOARDS, (target) => target.click(), DEALT.opening);
	await expect(page.getByRole('heading', { name: 'X wins on a line' })).toBeVisible();

	await page.getByRole('link', { name: 'Replay' }).click();
	await expect(page).toHaveURL(/\/replay\?game=[\w-]+$/);
	address = page.url();
	await expect(caption(page)).toHaveText('Start. X moves first.');
	await expect(stepped(page)).toHaveAttribute('data-focus', 'global');
	await expect(page.locator('[data-focus] .cell svg')).toHaveCount(0);

	// The dealt plies, marked in the caption, on the board and in the move list.
	const next = page.getByRole('button', { name: 'Next move' });
	await next.click();
	await ply(page, 1);
	await expect(caption(page)).toHaveText(`Move 1 of 55 · X (${notation(x)}) · dealt`);
	await expect(stepped(page)).toHaveAttribute('data-focus', x.slice(0, 2).join(''));
	await expect(page.getByRole('button', { name: `Cell ${notation(x)}, X, dealt` })).toHaveAttribute(
		'data-dealt'
	);
	await next.click();
	await expect(caption(page)).toHaveText(`Move 2 of 55 · O (${notation(o)}) · dealt`);
	await expect(page.getByRole('button', { name: `Cell ${notation(o)}, O, dealt` })).toHaveAttribute(
		'data-dealt'
	);
	await next.click();
	const first = parse(LINE_OF_MIDDLE_BOARDS)[0];
	await expect(caption(page)).toHaveText(new RegExp(`^Move 3 of 55 · X ${notation(first)} · `));
	await expect(
		page.getByRole('button', { name: `Cell ${notation(first)}, X`, exact: true })
	).not.toHaveAttribute('data-dealt');
	await expect(row(page, 1)).toContainText(`(${notation(x)})`);

	// A jump shows that ply's position: middle Board 1 open just before X wins it, won after.
	const won = page.getByRole('button', { name: /Won board 1$/ });
	const n = Number((await won.textContent())!.trim().split(/\s+/)[0]);
	await row(page, n - 1).click();
	await ply(page, n - 1);
	await page.getByRole('button', { name: 'Global', exact: true }).click();
	await expect(page.locator('[data-board="0"]')).toHaveAttribute('data-status', 'open');
	await won.click();
	await ply(page, n);
	const move = parse(LINE_OF_MIDDLE_BOARDS)[n - 3];
	await expect(stepped(page)).toHaveAttribute('data-focus', move.slice(0, 2).join(''));
	await expect(
		page.getByRole('button', { name: `Cell ${notation(move)}, X` }).locator('[data-last]')
	).toHaveCount(1);
	await page.getByRole('button', { name: 'Global', exact: true }).click();
	await expect(page.locator('[data-board="0"]')).toHaveAttribute('data-status', 'won-x');

	// Keys step and jump, and the address keeps the ply through a reload.
	await page.keyboard.press('End');
	await ply(page, 55);
	await expect(caption(page)).toHaveText(/^Move 55 of 55 · X /);
	await page.keyboard.press('Home');
	await ply(page, 0);
	await page.keyboard.press('ArrowRight');
	await ply(page, 1);
	await row(page, 30).click();
	await page.reload();
	await expect(caption(page)).toHaveText(/^Move 30 of 55 · /);

	// Autoplay steps once a second, until paused.
	await page.getByRole('button', { name: 'First move' }).click();
	await page.getByRole('button', { name: 'Play', exact: true }).click();
	await expect(page).toHaveURL(`${address}&ply=2`, { timeout: 3500 });
	await page.getByRole('button', { name: 'Pause', exact: true }).click();
	const paused = page.url();
	await page.waitForTimeout(1500);
	expect(page.url()).toBe(paused);

	// On a phone a jump lands on the move's Local board too.
	await page.setViewportSize({ width: 390, height: 844 });
	await row(page, 3).click();
	await expect(stepped(page)).toHaveAttribute('data-focus', first.slice(0, 2).join(''));

	// Home lists the finished game with the way to its Replay.
	await page.goto('/');
	const listed = page.locator('[data-recent="finished"]');
	await expect(listed).toContainText('X wins on a line · Replay');
	const { pathname, search } = new URL(address);
	await expect(listed).toHaveAttribute('href', pathname + search);
});

test('a replay link naming no saved game says so', async ({ page }) => {
	await page.goto('/replay?game=nothing-here');
	await expect(page.getByText('This game isn’t saved on this device.')).toBeVisible();
	await expect(page.getByRole('link', { name: 'Back to Home' })).toBeVisible();
});
