import { expect, test, type Page } from '@playwright/test';
import { DEALT, LINE_OF_MIDDLE_BOARDS, notation, parse } from '../lib/game/games.fixtures.js';
import { stepped, stepThrough } from './steps.js';

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

const [x, o] = parse(DEALT.opening);
const played = parse(LINE_OF_MIDDLE_BOARDS);

const caption = (page: Page) => page.locator('[data-caption]');

/** The move-list row for ply `n`. */
const row = (page: Page, n: number) =>
	page.getByRole('button', { name: new RegExp(`^${n} [XO] `) });

/** A Cell of the board shown, read-only in a replay, by its full name. */
const cell = (page: Page, name: string) => page.getByRole('img', { name, exact: true });

test.describe('the Replay of a finished Deep game with a Random opening', () => {
	test.describe.configure({ mode: 'serial' });

	let page: Page;
	/** The Replay's address, at the start. */
	let address: string;

	/** Opens the Replay at its start, once it has loaded. */
	const open = async () => {
		await page.goto(address);
		await expect(caption(page)).toHaveText('Start. X moves first.');
	};

	/** The address shows ply `n`. */
	const atPly = (n: number) => expect(page).toHaveURL(n ? `${address}&ply=${n}` : address);

	// One game, played once by click on a desktop, for every test here to replay.
	test.beforeAll(async ({ browser }, info) => {
		info.setTimeout(info.timeout * 3);
		page = await browser.newPage({
			baseURL: info.project.use.baseURL,
			viewport: { width: 1280, height: 720 },
			reducedMotion: 'reduce'
		});
		await dealFrom(page, DEALT.seed);
		await page.goto(`/play?rules=${DEALT.code}`);
		await expect(page.getByText(`(${notation(x)})`)).toBeVisible();
		await stepThrough(page, LINE_OF_MIDDLE_BOARDS, (target) => target.click(), DEALT.opening);
		await expect(page.getByRole('heading', { name: 'X wins on a line' })).toBeVisible();
		await page.getByRole('link', { name: 'Replay' }).click();
		await expect(page).toHaveURL(/\/replay\?game=[\w-]+$/);
		address = page.url();
	});

	test.afterAll(() => page.close());

	test('opens at the start from the game-over banner', async () => {
		await expect(page).toHaveTitle('Replay: Custom variant · Kokonoe');
		await expect(caption(page)).toHaveText('Start. X moves first.');
		await expect(stepped(page)).toHaveAttribute('data-focus', 'global');
		await expect(page.locator('[data-focus] .cell svg')).toHaveCount(0);
		await expect(page.getByRole('button', { name: 'Previous move' })).toHaveAttribute(
			'aria-disabled',
			'true'
		);
	});

	test('marks the dealt plies on the board, in the caption and in the move list', async () => {
		await open();
		const next = page.getByRole('button', { name: 'Next move' });
		await next.click();
		await atPly(1);
		await expect(caption(page)).toHaveText(`Move 1 of 55 · X (${notation(x)}) · dealt`);
		await expect(stepped(page)).toHaveAttribute('data-focus', x.slice(0, 2).join(''));
		await expect(cell(page, `Cell ${notation(x)}, X, dealt`)).toHaveAttribute('data-dealt');
		await next.click();
		await expect(caption(page)).toHaveText(`Move 2 of 55 · O (${notation(o)}) · dealt`);
		await expect(cell(page, `Cell ${notation(o)}, O, dealt`)).toHaveAttribute('data-dealt');
		await next.click();
		await expect(caption(page)).toHaveText(
			new RegExp(`^Move 3 of 55 · X ${notation(played[0])} · `)
		);
		await expect(cell(page, `Cell ${notation(played[0])}, X`)).not.toHaveAttribute('data-dealt');
		await expect(row(page, 1)).toContainText(`(${notation(x)})`);
	});

	test('jumps to any ply’s position, and the address keeps it through a reload', async () => {
		await open();
		// Middle Board 1 is open just before the move that wins it, and won after.
		const won = page.getByRole('button', { name: /Won board 1$/ });
		const n = Number((await won.textContent())!.trim().split(/\s+/)[0]);
		await row(page, n - 1).click();
		await atPly(n - 1);
		await page.getByRole('button', { name: 'Global', exact: true }).click();
		await expect(page.locator('[data-board="0"]')).toHaveAttribute('data-status', 'open');

		await won.click();
		await atPly(n);
		const move = played[n - 3];
		await expect(stepped(page)).toHaveAttribute('data-focus', move.slice(0, 2).join(''));
		await expect(cell(page, `Cell ${notation(move)}, X`).locator('[data-last]')).toHaveCount(1);
		await page.getByRole('button', { name: 'Global', exact: true }).click();
		await expect(page.locator('[data-board="0"]')).toHaveAttribute('data-status', 'won-x');

		await page.reload();
		await expect(caption(page)).toHaveText(new RegExp(`^Move ${n} of 55 · X `));
	});

	test('steps by key, and keeps the focus on a control pressed to the end', async () => {
		await open();
		await page.keyboard.press('End');
		await atPly(55);
		await expect(caption(page)).toHaveText(/^Move 55 of 55 · X /);
		await page.keyboard.press('Home');
		await atPly(0);
		await page.keyboard.press('ArrowRight');
		await atPly(1);

		const previous = page.getByRole('button', { name: 'Previous move' });
		await previous.focus();
		await page.keyboard.press('Enter');
		await atPly(0);
		await expect(previous).toHaveAttribute('aria-disabled', 'true');
		await expect(previous).toBeFocused();
	});

	test('plays a ply at a time, until paused', async () => {
		await open();
		await page.getByRole('button', { name: 'Play', exact: true }).click();
		await expect(caption(page)).toHaveAttribute('aria-live', 'off');
		await atPly(1);
		await page.getByRole('button', { name: 'Pause', exact: true }).click();
		await expect(page.getByRole('button', { name: 'Play', exact: true })).toBeVisible();
		await expect(caption(page)).toHaveAttribute('aria-live', 'polite');
	});

	test('lands each jump on the move’s Local board on a phone', async () => {
		await page.setViewportSize({ width: 390, height: 844 });
		await open();
		await row(page, 3).click();
		await expect(stepped(page)).toHaveAttribute('data-focus', played[0].slice(0, 2).join(''));
		await page.setViewportSize({ width: 1280, height: 720 });
	});

	test('is where Home’s Recent games opens the finished game', async () => {
		await open();
		await page.getByRole('link', { name: 'Home', exact: true }).click();
		await expect(page).toHaveTitle('Kokonoe');
		const listed = page.locator('[data-recent="finished"]');
		await expect(listed).toContainText('X wins on a line · Replay');
		const { pathname, search } = new URL(address);
		await expect(listed).toHaveAttribute('href', pathname + search);
	});
});

test('a replay link naming no saved game says so', async ({ page }) => {
	await page.goto('/replay?game=nothing-here');
	await expect(page.getByText('This game isn’t saved on this device.')).toBeVisible();
	await expect(page).toHaveTitle('This replay can’t open · Kokonoe');
	await expect(page.getByRole('link', { name: 'Back to Home' })).toBeVisible();
});
