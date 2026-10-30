import { readFile } from 'node:fs/promises';
import { createServer } from 'node:http';
import type { AddressInfo } from 'node:net';
import { extname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { expect, test, type Page } from '@playwright/test';
import { TOP_ROW } from '../lib/game/games.fixtures.js';
import { caption, cell, marked, play } from './steps.js';

// The other tests keep the service worker off; these are what it's for.
test.use({ serviceWorkers: 'allow' });

const BUILD = fileURLToPath(new URL('../../build/', import.meta.url));

const TYPES: Record<string, string> = {
	'.html': 'text/html',
	'.js': 'text/javascript',
	'.css': 'text/css',
	'.json': 'application/json',
	'.wasm': 'application/wasm',
	'.woff2': 'font/woff2',
	'.txt': 'text/plain'
};

/**
 * The build, served on its own port, so a test can shut the host down to prove a page came from
 * the service worker, or deploy a "next" version to it: the same files with the version renamed,
 * as a new build changes it in the service worker and in the page every route falls back to.
 * Playwright's routing can't stand in, since it never sees the browser fetching a service
 * worker's script. Only for these tests: it takes any path inside the build, and answers
 * anything else with the fallback page.
 */
const host = async () => {
	let version: string | undefined;
	let stuck = false;
	const server = createServer(async (request, response) => {
		const file = join(BUILD, new URL(request.url ?? '/', 'http://host').pathname);
		const body = await readFile(file).catch(() => readFile(join(BUILD, '200.html')));
		const type = TYPES[extname(file)] ?? 'text/html';
		if (stuck && type === 'text/html') return; // connected, but never answering
		const renamed = version !== undefined && type.startsWith('text/');
		response
			.writeHead(200, { 'content-type': type })
			.end(renamed ? body.toString().replaceAll(version!, 'next') : body);
	});
	await new Promise<void>((resolve) => server.listen(0, resolve));
	const origin = `http://localhost:${(server.address() as AddressInfo).port}`;
	return {
		at: (path: string) => origin + path,
		/** Renames `cache`'s version to `next` in everything served from now on. */
		deploy: (cache: string) => (version = cache.replace('kokonoe-', '')),
		/** Leaves every page request unanswered from now on, as a network that's stuck does. */
		stick: () => (stuck = true),
		shutDown: () => {
			server.close();
			server.closeAllConnections();
		}
	};
};

/** Opens Home, and waits until the service worker has kept the build and controls the page. */
const install = async (page: Page, home: string) => {
	await page.goto(home);
	await page.evaluate(async () => {
		await navigator.serviceWorker.ready;
		if (navigator.serviceWorker.controller) return;
		await new Promise((resolve) =>
			navigator.serviceWorker.addEventListener('controllerchange', resolve, { once: true })
		);
	});
};

const caches = (page: Page) => page.evaluate(() => globalThis.caches.keys());

test('every page works with no network', async ({ page, context }) => {
	const site = await host();
	await install(page, site.at('/'));
	site.shutDown();
	await context.setOffline(true);

	await page.goto(site.at('/play'));
	await play(page, TOP_ROW);
	await expect(page.getByRole('heading', { name: 'X wins on a line' })).toBeVisible();
	const [download] = await Promise.all([
		page.waitForEvent('download'),
		page.getByRole('button', { name: 'Export as .utt' }).click()
	]);
	const utt = await readFile(await download.path(), 'utf8');

	await page.goto(site.at('/import'));
	await page.getByLabel('Paste a .utt or JSON game').fill(utt);
	await page.getByRole('button', { name: 'Open the replay' }).click();
	await expect(page).toHaveURL(/\/replay$/);
	await page.getByRole('button', { name: 'Next move' }).click();
	await expect(caption(page)).toHaveText(/^Move 1 of 17 /);
	await page.reload();
	await expect(caption(page), 'a reload opens too').toHaveText(/^Move 1 of 17 /);

	await page.goto(site.at('/play?vs=computer&level=1&seat=X'));
	await cell(page, '44').click();
	await expect(marked(page), 'the computer answered').toHaveCount(2);
});

test('a new build takes over on the next reload', async ({ page, context }) => {
	test.setTimeout(60_000);
	const site = await host();
	try {
		await install(page, site.at('/'));
		const [current] = await caches(page);
		site.deploy(current);
		await page.reload();
		await expect.poll(() => caches(page), { timeout: 30_000 }).toEqual(['kokonoe-next']);
	} finally {
		site.shutDown();
	}

	// The reload after that boots the new build from the new cache alone.
	await context.setOffline(true);
	await page.reload();
	await expect(page.getByRole('link', { name: 'Import game' })).toBeVisible();
	const shell = await page.evaluate(async () => {
		const kept = await globalThis.caches.match(location.origin + '/');
		return (await kept?.text()) ?? '';
	});
	expect(shell).toContain('"next"');
});

test('a stuck network opens the kept page within seconds', async ({ page }) => {
	const site = await host();
	try {
		await install(page, site.at('/'));
		site.stick();
		const start = Date.now();
		await page.goto(site.at('/play'));
		await expect(page.getByRole('group', { name: 'The Global board' })).toBeVisible();
		expect(Date.now() - start, 'about 4 s, not the browser’s minutes').toBeLessThan(10_000);
	} finally {
		site.shutDown();
	}
});
