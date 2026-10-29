import { afterEach, expect, it, vi } from 'vitest';
import { render } from 'vitest-browser-svelte';
import ExportButtons from './ExportButtons.svelte';
import type { GameFile } from './gamefile.js';
import { SHELF } from './shelf.js';

const file: GameFile = {
	record: { rules_version: 1, ruleset: SHELF[0].ruleset, seed: '00000000005eed00', moves: [40] },
	variant: 'Standard',
	seats: { X: 'Player 1', O: 'Player 2' },
	date: '2026-10-28',
	opening: [],
	end: null
};

afterEach(() => {
	vi.restoreAllMocks();
});

/** The name and text of the next file the page offers to download. */
const downloaded = () => {
	const offered: { name: string; blob: Blob }[] = [];
	vi.spyOn(URL, 'createObjectURL').mockImplementation((blob) => {
		offered.push({ name: '', blob: blob as Blob });
		return 'blob:test';
	});
	vi.spyOn(HTMLAnchorElement.prototype, 'click').mockImplementation(function (
		this: HTMLAnchorElement
	) {
		offered[offered.length - 1].name = this.download;
	});
	return offered;
};

it('downloads the game as .utt or as JSON, named by its day and variant', async () => {
	const offered = downloaded();
	const screen = await render(ExportButtons, { file });
	await screen.getByRole('button', { name: 'Export as .utt' }).click();
	await screen.getByRole('button', { name: 'Export as JSON' }).click();
	expect(offered.map((download) => download.name)).toEqual([
		'kokonoe-2026-10-28-standard.utt',
		'kokonoe-2026-10-28-standard.json'
	]);
	expect(await offered[0].blob.text()).toMatch(/^\[Variant "Standard"\]\n/);
	expect(JSON.parse(await offered[1].blob.text()).moves).toEqual([40]);
});

it('copies the .utt text, and says so', async () => {
	const write = vi.spyOn(navigator.clipboard, 'writeText').mockResolvedValue();
	const screen = await render(ExportButtons, { file });
	await screen.getByRole('button', { name: 'Copy .utt' }).click();
	await expect.element(screen.getByRole('status')).toHaveTextContent('Copied');
	expect(write.mock.calls[0][0]).toMatch(/\n1\. 5·5 \*\n$/);
});

it('says when the copy fails', async () => {
	vi.spyOn(navigator.clipboard, 'writeText').mockRejectedValue(new Error('denied'));
	const screen = await render(ExportButtons, { file });
	await screen.getByRole('button', { name: 'Copy .utt' }).click();
	await expect
		.element(screen.getByRole('status'))
		.toHaveTextContent('Couldn’t copy. Export as .utt instead.');
});
