import { readFile } from 'node:fs/promises';
import { expect, test, type Page } from '@playwright/test';
import { DEALT, LINE_OF_MIDDLE_BOARDS, notation, parse } from '../lib/game/games.fixtures.js';
import { dayOf } from '../lib/game/listing.js';
import { caption, dealFrom, stepThrough } from './steps.js';

const [x] = parse(DEALT.opening);

/** The `.utt` tags the finished `DEALT` game must open with, pinned in order and spelling. */
const TAGS = [
	'[Variant "Custom variant"]',
	'[Ruleset "d3+rand2"]',
	`[Seed "${DEALT.seed}"]`,
	'[RulesVersion "1"]',
	'[X "Player 1"]',
	'[O "Player 2"]',
	`[Date "${dayOf(Date.now())}"]`,
	'[TimeControl "-"]',
	'[Result "1-0"]',
	'[Termination "line"]'
].join('\n');

test.describe('a finished Deep game with a Random opening, as a file', () => {
	test.describe.configure({ mode: 'serial' });

	let page: Page;
	/** The game's `.utt` and JSON, as the banner exports them. */
	let utt: string;
	let json: string;

	// One game, played once by click on a desktop, for every test here to export and import.
	test.beforeAll(async ({ browser }, info) => {
		info.setTimeout(info.timeout * 3);
		const context = await browser.newContext({
			baseURL: info.project.use.baseURL,
			viewport: { width: 1280, height: 720 },
			reducedMotion: 'reduce',
			permissions: ['clipboard-read', 'clipboard-write']
		});
		page = await context.newPage();
		await dealFrom(page, DEALT.seed);
		await page.goto(`/play?rules=${DEALT.code}`);
		await expect(page.getByText(`(${notation(x)})`)).toBeVisible();
		await stepThrough(page, LINE_OF_MIDDLE_BOARDS, (target) => target.click(), DEALT.opening);
		await expect(page.getByRole('heading', { name: 'X wins on a line' })).toBeVisible();
	});

	test.afterAll(() => page.context().close());

	/** Downloads by pressing `name`, and returns the file's name and text. */
	const download = async (name: string) => {
		const [file] = await Promise.all([
			page.waitForEvent('download'),
			page.getByRole('button', { name }).click()
		]);
		return { name: file.suggestedFilename(), text: await readFile(await file.path(), 'utf8') };
	};

	test('exports from the game-over banner as .utt and JSON', async () => {
		const day = dayOf(Date.now());
		const text = await download('Export as .utt');
		expect(text.name).toBe(`kokonoe-${day}-custom-variant.utt`);
		expect(text.text.startsWith(`${TAGS}\n\n1. (8·1·3) (1·3·9) 2. 3·9·1 `)).toBe(true);
		expect(text.text.endsWith(' 1-0\n')).toBe(true);
		utt = text.text;

		const record = await download('Export as JSON');
		expect(record.name).toBe(`kokonoe-${day}-custom-variant.json`);
		expect(JSON.parse(record.text)).toMatchObject({ name: 'Custom variant', seed: DEALT.seed });
		expect(JSON.parse(record.text).moves).toHaveLength(53);
		json = record.text;
	});

	test('copies the .utt text', async () => {
		await page.getByRole('button', { name: 'Copy .utt' }).click();
		await expect(page.getByRole('status').filter({ hasText: 'Copied' })).toBeVisible();
		// The system clipboard may turn each line ending into CRLF, as Windows does.
		const copied = await page.evaluate(() => navigator.clipboard.readText());
		expect(copied.replaceAll('\r\n', '\n')).toBe(utt);
	});

	test('imports a pasted .utt into the Replay viewer, which a reload keeps', async () => {
		await page.goto('/');
		await page.getByRole('link', { name: 'Import game' }).click();
		await expect(page.getByRole('heading', { name: 'Import a game' })).toBeVisible();
		await page.getByLabel('Paste a .utt or JSON game').fill(utt);
		await page.getByRole('button', { name: 'Open the replay' }).click();
		await expect(page).toHaveURL(/\/replay$/);
		await expect(page.getByRole('banner')).toContainText('Imported');
		await expect(caption(page)).toHaveText('Start. X moves first.');
		await expect(page.getByRole('heading', { name: 'X wins on a line' })).toBeVisible();
		await page.getByRole('button', { name: 'Next move' }).click();
		await expect(caption(page)).toHaveText(`Move 1 of 55 · X (${notation(x)}) · dealt`);
		await expect(page).toHaveURL(/\/replay\?ply=1$/);

		await page.reload();
		await expect(caption(page)).toHaveText(`Move 1 of 55 · X (${notation(x)}) · dealt`);
		await expect(page.locator('[data-variant]')).toHaveText('Custom variant');
	});

	test('imports a chosen JSON file', async () => {
		await page.goto('/import');
		await page.locator('input[type="file"]').setInputFiles({
			name: 'game.json',
			mimeType: 'application/json',
			buffer: Buffer.from(json)
		});
		await expect(page).toHaveURL(/\/replay$/);
		await expect(page.locator('[data-variant]')).toHaveText('Custom variant');
		await page.keyboard.press('End');
		await expect(caption(page)).toHaveText(/^Move 55 of 55 · X /);
	});

	test('imports a dropped file', async () => {
		await page.goto('/import');
		const dropped = await page.evaluateHandle((text) => {
			const transfer = new DataTransfer();
			transfer.items.add(new File([text], 'game.utt', { type: 'text/plain' }));
			return transfer;
		}, utt);
		await page.locator('[data-drop]').dispatchEvent('drop', { dataTransfer: dropped });
		await expect(page).toHaveURL(/\/replay$/);
		await expect(caption(page)).toHaveText('Start. X moves first.');
	});

	test('names the first bad move of a file it refuses', async () => {
		await page.goto('/import');
		const broken = utt.replace('2. 3·9·1', '2. 1·1·1');
		await page.getByLabel('Paste a .utt or JSON game').fill(broken);
		await page.getByRole('button', { name: 'Open the replay' }).click();
		const box = page.getByLabel('Paste a .utt or JSON game');
		await expect(page.locator('[data-problem]')).toHaveText('Move 3, X 1·1·1, isn’t legal.');
		await expect(page).toHaveURL(/\/import$/);
		await expect(box).toBeFocused();
		await expect(box).toHaveAttribute('aria-invalid', 'true');
		await expect(box).toHaveAccessibleDescription('Move 3, X 1·1·1, isn’t legal.');

		// The same file chosen twice, as after fixing it, is read both times.
		const file = { name: 'game.utt', mimeType: 'text/plain', buffer: Buffer.from(broken) };
		const input = page.locator('input[type="file"]');
		await input.setInputFiles(file);
		await expect(input).toHaveValue('');
		await input.setInputFiles({ ...file, buffer: Buffer.from(utt) });
		await expect(page).toHaveURL(/\/replay$/);
	});
});
