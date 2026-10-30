import { appendFileSync } from 'node:fs';
import { brotliCompressSync, constants } from 'node:zlib';
import { expect, test } from '@playwright/test';

/**
 * The game screen at its heaviest: against the computer, which opens as X, so the page also loads
 * the computer's Worker.
 */
const GAME_SCREEN = '/play?vs=computer&level=1&seat=O';

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

test('the game screen loads at most 100 kB brotli of JS and WASM, the service worker’s included', async ({
	page
}) => {
	const loads = new Map<string, Promise<number>>();
	const failed: string[] = [];
	page.on('response', (response) => {
		const path = new URL(response.url()).pathname;
		if (!/\.(m?js|wasm)$/.test(path) || loads.has(path)) return;
		// A 404 body is not the file, so it mustn't count as it.
		if (response.ok()) loads.set(path, response.body().then(brotli));
		else failed.push(`${path}: ${response.status()}`);
	});
	await page.goto(GAME_SCREEN);
	await page.waitForLoadState('networkidle');

	const files = new Map<string, number>();
	for (const [path, size] of loads) files.set(path, await size);
	// The tests keep the service worker off, so it's fetched here. What it keeps for offline
	// loads after the page, in the worker, and isn't the screen's.
	const worker = await page.request.get('/service-worker.js');
	expect(worker.ok(), 'the service worker is built').toBe(true);
	files.set('/service-worker.js', brotli(await worker.body()));
	const total = [...files.values()].reduce((sum, size) => sum + size, 0);
	report(files, total);
	expect(failed, 'every JS and WASM file loaded').toEqual([]);
	expect(files.size, 'the screen loaded some JS').toBeGreaterThan(0);
	expect(
		[...files.keys()].some((path) => path.endsWith('.wasm')),
		'the engine counts'
	).toBe(true);
	expect(total).toBeLessThanOrEqual(BUDGET);
});
