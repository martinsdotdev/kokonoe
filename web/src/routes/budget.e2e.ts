import { appendFileSync } from 'node:fs';
import { brotliCompressSync, constants } from 'node:zlib';
import { expect, test } from '@playwright/test';

/** The route of the game screen. A stub that loads the engine stands in until KOK-37. */
const GAME_SCREEN = '/play';

/** ADR-0001's startup budget for the game screen's JS plus WASM, in brotli bytes. */
const BUDGET = 100_000;

const brotli = (body: Buffer) =>
	brotliCompressSync(body, { params: { [constants.BROTLI_PARAM_QUALITY]: 11 } }).length;

const report = (files: Map<string, number>, total: number) => {
	const rows = [...files].sort(([, a], [, b]) => b - a);
	const table = [
		`### Game screen: ${total.toLocaleString('en')} of ${BUDGET.toLocaleString('en')} bytes brotli`,
		'',
		'| File | Brotli bytes |',
		'| -- | --: |',
		...rows.map(([path, size]) => `| \`${path}\` | ${size.toLocaleString('en')} |`),
		''
	].join('\n');
	const summary = process.env.GITHUB_STEP_SUMMARY;
	if (summary) appendFileSync(summary, table + '\n');
	else console.log(table);
};

test('the game screen loads at most 100 kB brotli of JS and WASM', async ({ page }) => {
	const loads = new Map<string, Promise<number>>();
	page.on('response', (response) => {
		const path = new URL(response.url()).pathname;
		if (/\.(m?js|wasm)$/.test(path) && !loads.has(path)) {
			loads.set(path, response.body().then(brotli));
		}
	});
	await page.goto(GAME_SCREEN);
	await page.waitForLoadState('networkidle');

	const files = new Map<string, number>();
	for (const [path, size] of loads) files.set(path, await size);
	const total = [...files.values()].reduce((sum, size) => sum + size, 0);
	report(files, total);
	expect(files.size, 'the screen loaded some JS').toBeGreaterThan(0);
	expect(
		[...files.keys()].some((path) => path.endsWith('.wasm')),
		'the engine counts'
	).toBe(true);
	expect(total).toBeLessThanOrEqual(BUDGET);
});
