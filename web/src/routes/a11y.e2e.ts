import AxeBuilder from '@axe-core/playwright';
import { expect, test, type Page } from '@playwright/test';
import { INTO_A_WON_BOARD, TOP_ROW, cells } from '../lib/game/games.fixtures.js';
import { HOST, JOINER, ROOM, awayNow, clocksNow, fakeRoom } from './room.fixtures.js';
import { focusBoard, play, typeMoves } from './steps.js';

/** WCAG 2.2 AA (spec §7.7), with the A and AA rules of every version before it. */
const TAGS = ['wcag2a', 'wcag2aa', 'wcag21a', 'wcag21aa', 'wcag22aa'];

/** X wins Local board 1, then O Sends X back into it: a Free move. */
const FREE_MOVE = '03 30 04 40 05 50';

/** Scans the page as it stands, and lists each rule it breaks with where and why. */
const scan = async (page: Page, what: string) => {
	const { violations } = await new AxeBuilder({ page }).withTags(TAGS).analyze();
	const found = violations.map((violation) =>
		[
			`${violation.id} (${violation.impact}): ${violation.help}`,
			...violation.nodes.slice(0, 5).map((node) => `  ${node.target.join(' ')}`),
			`  ${violation.helpUrl}`
		].join('\n')
	);
	expect(found, `${what} passes axe`).toEqual([]);
};

for (const colorScheme of ['light', 'dark'] as const) {
	test.describe(`in the ${colorScheme === 'light' ? 'paper' : 'night'} palette`, () => {
		test.use({ colorScheme });

		test('Home, New game and Import', async ({ page }) => {
			await page.route('/api/session', (route) =>
				route.fulfill({ json: { handle: 'guest-4821' } })
			);
			await page.goto('/');
			await expect(page.locator('[data-account]')).toBeVisible();
			await scan(page, 'Home');
			await page.goto('/new-game');
			await expect(page.getByRole('heading').first()).toBeVisible();
			await scan(page, 'New game');
			await page.goto('/new-game?vs=friend');
			await page.getByRole('button', { name: 'Create invite Room' }).click();
			await expect(page.getByRole('alert')).toBeVisible();
			await scan(page, 'New game, inviting a friend, offline');
			await page.goto('/r/6f1c0a9b3e2d4c5a7b8e');
			await expect(page.getByRole('heading', { name: 'You’re offline' })).toBeVisible();
			await scan(page, 'a Room’s link offline');
			await page.goto('/r/abc');
			await expect(page.getByRole('heading', { name: 'This game doesn’t exist' })).toBeVisible();
			await scan(page, 'a Room that doesn’t exist');
			await page.goto('/import');
			await expect(page.getByRole('heading', { name: 'Import a game' })).toBeVisible();
			await scan(page, 'Import');
		});

		test('the game screen at depth 2, and the Replay viewer', async ({ page }) => {
			await page.goto('/play');
			await expect(page.getByRole('group', { name: 'The Global board' })).toBeVisible();
			await scan(page, 'the start');
			await play(page, FREE_MOVE);
			await expect(page.locator('[data-stamp]')).toBeVisible();
			await scan(page, 'a Free move');
			await page.goto('/play');
			await play(page, TOP_ROW);
			await expect(page.getByRole('heading', { name: 'X wins on a line' })).toBeVisible();
			await scan(page, 'the game over');
			await page.getByRole('link', { name: 'Replay' }).click();
			await expect(page.locator('[data-caption]')).toBeVisible();
			await scan(page, 'the Replay viewer');
		});

		test('an invite Room: the joiner’s card, the host’s wait, and the game', async ({ page }) => {
			const room = await fakeRoom(page, {
				you: 'O',
				players: { X: HOST, O: JOINER },
				moves: [],
				card: {
					host: HOST,
					variant: 'Standard',
					rules: 'std',
					players: { X: HOST, O: null },
					you: null,
					takebacks: false,
					clock: null,
					spectators: false
				}
			});
			await page.goto(`/r/${ROOM}`);
			await expect(page.getByRole('button', { name: 'Accept' })).toBeVisible();
			await scan(page, 'the joiner’s card');
			await page.getByRole('button', { name: 'Accept' }).click();
			await expect(page.getByRole('group', { name: 'The Global board' })).toBeVisible();
			await scan(page, 'the game');
			room.close(1011);
			await expect(page.locator('[data-trouble]')).toBeVisible();
			await scan(page, 'reconnecting');

			await page.unrouteAll();
			await fakeRoom(page, { you: 'X', players: { X: HOST, O: null }, moves: [] });
			await page.goto(`/r/${ROOM}`);
			await expect(page.getByRole('button', { name: 'Copy link' })).toBeVisible();
			await scan(page, 'the host’s wait');
		});

		test('a watched Room: the card’s Watch, the wait for a second player, and the game', async ({
			page
		}) => {
			const seats = { X: HOST, O: null };
			const room = await fakeRoom(page, {
				you: null,
				players: seats,
				moves: [],
				card: {
					host: HOST,
					variant: 'Standard',
					rules: 'std',
					players: seats,
					you: null,
					takebacks: false,
					clock: null,
					spectators: true
				},
				snapshot: { watching: 2 }
			});
			await page.goto(`/r/${ROOM}`);
			await expect(page.getByRole('button', { name: 'Watch' })).toBeVisible();
			await scan(page, 'the joiner’s card, with Watch');
			await page.getByRole('button', { name: 'Watch' }).click();
			await expect(page.locator('[data-unseated]')).toBeVisible();
			await scan(page, 'a watched Room’s wait for a second player');
			room.join('guest-0007');
			await expect(page.locator('[data-unseated]')).toHaveCount(0);
			await expect(page.getByRole('region', { name: 'Turn' })).toBeVisible();
			await scan(page, 'the watched game');
		});

		test('a Room’s offers, and its rematch', async ({ page }) => {
			const top = cells(TOP_ROW);
			const players = { X: HOST, O: JOINER };
			const snapshot = { takebacks: true };
			let room = await fakeRoom(page, { you: 'X', players, moves: top.slice(0, 2), snapshot });
			await page.goto(`/r/${ROOM}`);
			await expect(page.getByRole('button', { name: 'Ask takeback' })).toBeVisible();
			room.say({ type: 'Offer', kind: 'Takeback', by: 'O', lapses_in: 15_000 });
			await expect(page.locator('[data-left]')).toBeVisible();
			await scan(page, 'a takeback asked');
			room.say({ type: 'OfferAnswered', kind: 'Takeback', accepted: false });
			await page.getByRole('button', { name: 'Offer draw' }).click();
			await expect(page.getByText(/^You offered a draw/)).toBeVisible();
			await scan(page, 'a draw offered, waiting');
			room.say({ type: 'OfferAnswered', kind: 'Draw', accepted: false });
			await expect(page.locator('[data-notice]')).toBeVisible();
			await page.getByRole('button', { name: 'Resign' }).click();
			await expect(page.getByRole('button', { name: 'Yes, resign' })).toBeVisible();
			await scan(page, 'the notice, and resigning');

			await page.unrouteAll();
			room = await fakeRoom(page, { you: 'X', players, moves: top });
			await page.goto(`/r/${ROOM}`);
			await expect(page.getByRole('button', { name: 'Rematch' })).toBeVisible();
			room.say({ type: 'Offer', kind: 'Rematch', by: 'O', lapses_in: 60_000 });
			await expect(page.getByRole('button', { name: 'Accept' })).toBeVisible();
			await scan(page, 'a rematch offered');

			await page.unrouteAll();
			await fakeRoom(page, {
				you: 'X',
				players,
				moves: top,
				snapshot: { rematch: 'b'.repeat(20) }
			});
			await page.goto(`/r/${ROOM}`);
			await expect(page.getByRole('link', { name: 'Play the rematch' })).toBeVisible();
			await scan(page, 'the rematch made');
		});

		test('a timed Room: its clocks running low, a loss on time, and an abort', async ({ page }) => {
			const top = cells(TOP_ROW);
			const players = { X: HOST, O: JOINER };
			const room = await fakeRoom(page, {
				you: 'X',
				players,
				moves: top.slice(0, 2),
				snapshot: { clock: '3+2', clocks: clocksNow({ x_ms: 15_000, o_ms: 12_000, running: 'X' }) }
			});
			await page.goto(`/r/${ROOM}`);
			await expect(page.locator('[data-low]')).toHaveCount(2);
			await scan(page, 'the clocks running low');
			room.say({ type: 'Ended', result: '0-1', termination: 'timeout' });
			await expect(page.getByRole('heading', { name: 'X ran out of time. O wins.' })).toBeVisible();
			await scan(page, 'a loss on time');

			await page.unrouteAll();
			const aborting = await fakeRoom(page, {
				you: 'X',
				players,
				moves: [],
				snapshot: { clock: '3+2', clocks: clocksNow({ aborts_at: Date.now() + 30_000 }) }
			});
			await page.goto(`/r/${ROOM}`);
			await expect(page.locator('[data-aborts]')).toBeVisible();
			await scan(page, 'a first move awaited');
			aborting.say({ type: 'Aborted', reason: 'FirstMoveTimeout' });
			await expect(page.getByRole('heading', { name: 'Game aborted' })).toBeVisible();
			await scan(page, 'the game aborted');
		});

		test('a Room with the other player away: the Claim, and the ends it leads to', async ({
			page
		}) => {
			const top = cells(TOP_ROW);
			const players = { X: HOST, O: JOINER };
			const room = await fakeRoom(page, {
				you: 'X',
				players,
				moves: top.slice(0, 2),
				snapshot: { presence: { X: null, O: awayNow(65_000, 0) } }
			});
			await page.goto(`/r/${ROOM}`);
			await expect(page.getByRole('region', { name: 'Claim' })).toBeVisible();
			await expect(page.locator('[data-away]')).toBeVisible();
			await scan(page, 'a Claim due, and the player away');
			room.say({ type: 'Ended', result: '1-0', termination: 'claim' });
			await expect(
				page.getByRole('heading', { name: 'O left the game. X claimed the win.' })
			).toBeVisible();
			await scan(page, 'a claimed win');

			await page.unrouteAll();
			await fakeRoom(page, {
				you: 'X',
				players,
				moves: top.slice(0, 2),
				snapshot: { end: { result: null, termination: 'abandoned' } }
			});
			await page.goto(`/r/${ROOM}`);
			await expect(page.getByRole('heading', { name: 'Game abandoned' })).toBeVisible();
			await scan(page, 'an abandoned game');
		});

		test('the game screen in Deep', async ({ page }) => {
			await page.goto('/play?rules=d3');
			await expect(page.locator('[data-focus="global"]')).toBeVisible();
			await scan(page, 'the start');
			await focusBoard(page);
			await typeMoves(page, INTO_A_WON_BOARD);
			await expect(page.locator('[data-stamp]')).toBeVisible();
			await expect(page.locator('[data-settled]')).toBeVisible();
			await scan(page, 'a Free move inside a middle Board');
		});
	});
}
