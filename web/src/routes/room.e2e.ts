import { expect, test, type Page } from '@playwright/test';
import { TOP_ROW, cells } from '../lib/game/games.fixtures.js';
import { HOST, JOINER, ROOM, fakeRoom } from './room.fixtures.js';
import { cell, focusBoard, marked, settledFocus, stepThrough, typeMoves } from './steps.js';

const top = cells(TOP_ROW);
const digits = TOP_ROW.split(' ');

const board = (page: Page) => page.getByRole('group', { name: 'The Global board' });

test('a host invites a friend from Home, and plays once they join', async ({ context, page }) => {
	await context.grantPermissions(['clipboard-read', 'clipboard-write']);
	const room = await fakeRoom(page, { you: 'X', players: { X: HOST, O: null }, moves: [] });
	await page.goto('/');
	await page.getByRole('link', { name: 'Invite a friend' }).click();
	await expect(page).toHaveURL(/\/new-game\?vs=friend$/);
	await expect(page.getByRole('radio', { name: /^A friend/ })).toBeChecked();
	await expect(page.getByRole('radio', { name: 'Random' })).toBeChecked();
	await page.getByRole('button', { name: 'Create invite Room' }).click();

	await expect(page).toHaveURL(`/r/${ROOM}`);
	await expect(page.getByRole('heading', { name: 'Waiting for your friend' })).toBeVisible();
	await expect(page).toHaveTitle('Waiting for your friend · Kokonoe');
	const link = `${new URL(page.url()).origin}/r/${ROOM}`;
	await expect(page.getByLabel('Invite link')).toHaveValue(link);
	await page.getByRole('button', { name: 'Copy link' }).click();
	await expect(page.getByRole('status')).toHaveText('Link copied.');
	expect(await page.evaluate(() => navigator.clipboard.readText())).toBe(link);

	room.join(JOINER);
	await expect(board(page)).toBeVisible();
	await expect(page.getByRole('banner').getByText(`vs ${JOINER}`)).toBeVisible();
});

test('a move waits for the server before it shows', async ({ page }) => {
	const room = await fakeRoom(page, { you: 'X', players: { X: HOST, O: JOINER }, moves: [] });
	await page.goto(`/r/${ROOM}`);
	await expect(board(page)).toBeVisible();

	await cell(page, digits[0]).click();
	await expect.poll(() => room.moves()).toEqual([{ type: 'Move', ply: 0, cell: top[0] }]);
	await expect(marked(page)).toHaveCount(0);
	await cell(page, digits[2]).click({ force: true });
	room.moved(0, top[0]);
	await expect(marked(page)).toHaveCount(1);
	expect(room.moves(), 'nothing sent while it waited').toHaveLength(1);

	room.moved(1, top[1]);
	await expect(marked(page)).toHaveCount(2);
});

test('an illegal tap flashes and sends nothing', async ({ page }) => {
	const room = await fakeRoom(page, { you: 'X', players: { X: HOST, O: JOINER }, moves: [] });
	await page.goto(`/r/${ROOM}`);
	await expect(board(page)).toBeVisible();
	room.moved(0, top[0]);
	room.moved(1, top[1]);
	await expect(marked(page)).toHaveCount(2);
	// X is Sent to Board 1; a Cell of Board 6 isn't X's to play.
	await cell(page, '60').click({ force: true });
	await expect(
		page.getByRole('group', { name: 'Board 1, Sent here, play here', exact: true })
	).toHaveAttribute('data-flash');
	expect(room.moves()).toEqual([]);
});

test('a move can be typed, as offline', async ({ page }) => {
	const room = await fakeRoom(page, { you: 'X', players: { X: HOST, O: JOINER }, moves: [] });
	await page.goto(`/r/${ROOM}`);
	await expect(board(page)).toBeVisible();
	await focusBoard(page);
	await typeMoves(page, digits[0]);
	await expect.poll(() => room.moves()).toEqual([{ type: 'Move', ply: 0, cell: top[0] }]);
});

test.describe('on a phone', () => {
	test.use({ viewport: { width: 390, height: 844 }, hasTouch: true, isMobile: true });

	test('a tapped move waits for the server, then lands', async ({ page }) => {
		const room = await fakeRoom(page, { you: 'X', players: { X: HOST, O: JOINER }, moves: [] });
		await page.goto(`/r/${ROOM}`);
		await stepThrough(page, digits[0], (target) => target.tap());
		await expect.poll(() => room.moves()).toEqual([{ type: 'Move', ply: 0, cell: top[0] }]);
		await expect(marked(page)).toHaveCount(0);
		room.moved(0, top[0]);
		// The move landed: the view steps to the Board it Sent O to.
		await expect.poll(() => settledFocus(page)).toEqual([3]);
	});
});

test('a host whose connection drops while waiting is told', async ({ page }) => {
	const room = await fakeRoom(page, { you: 'X', players: { X: HOST, O: null }, moves: [] });
	await page.goto(`/r/${ROOM}`);
	await expect(page.getByRole('button', { name: 'Copy link' })).toBeVisible();
	room.close(1011);
	await expect(page.getByRole('status')).toHaveText('Connection lost. Reconnecting…');
	await expect.poll(() => room.hellos()).toBe(2);
	await expect(page.getByRole('status')).toHaveText('');
	room.join(JOINER);
	await expect(board(page)).toBeVisible();
	await expect(page.locator('main h1'), 'focus follows the game in').toBeFocused();
});

test('the sheet says so when the server can’t make a Room', async ({ page }) => {
	await page.goto('/new-game?vs=friend');
	await page.getByRole('button', { name: 'Create invite Room' }).click();
	await expect(page.getByRole('alert')).toHaveText(
		'Kokonoe’s server couldn’t make the Room. Invite a friend once you’re online.'
	);
	await expect(page).toHaveURL(/\/new-game/);
});

test('a Room’s link offline offers to try again', async ({ page }) => {
	await page.goto(`/r/${ROOM}`);
	await expect(page.getByRole('heading', { name: 'You’re offline' })).toBeVisible();
	await expect(page.getByRole('button', { name: 'Try again' })).toBeVisible();
});

test('the sheet loads no engine, inviting a friend', async ({ page }) => {
	const loaded: string[] = [];
	const scripts: Promise<string>[] = [];
	page.on('response', (response) => {
		const path = new URL(response.url()).pathname;
		if (!/\.(m?js|wasm)$/.test(path)) return;
		loaded.push(path);
		scripts.push(response.text());
	});
	await page.goto('/new-game?vs=friend');
	await expect(page.getByRole('button', { name: 'Create invite Room' })).toBeVisible();
	await page.waitForLoadState('networkidle');
	expect(loaded.filter((path) => path.endsWith('.wasm'))).toEqual([]);
	for (const script of await Promise.all(scripts)) expect(script).not.toContain('kokonoe_wasm_bg');
});

test('a joiner reads the card, and joins on Accept or leaves on Decline', async ({ page }) => {
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
			takebacks: false
		}
	});
	await page.goto(`/r/${ROOM}`);
	await expect(
		page.getByRole('heading', { name: `${HOST} invites you to a game of Standard` })
	).toBeVisible();
	await expect(page).toHaveTitle(`${HOST} invites you · Kokonoe`);
	const chips = page.getByRole('list', { name: 'The game' }).getByRole('listitem');
	await expect(chips).toHaveText(['Preset', 'Casual', 'Untimed']);
	expect(room.hellos(), 'no socket before Accept').toBe(0);
	await page.getByRole('link', { name: 'Decline' }).click();
	await expect(page).toHaveURL('/');

	await page.goto(`/r/${ROOM}`);
	await page.getByRole('button', { name: 'Accept' }).click();
	await expect(board(page)).toBeVisible();
	await expect(page.locator('main h1'), 'focus follows the page into the game').toBeFocused();
	expect(room.said[0]).toEqual({
		type: 'Hello',
		protocol: 2,
		rules_version: 1,
		room: ROOM,
		role: 'Player'
	});
	await expect(page.getByRole('complementary', { name: 'Players and turn' })).toContainText('You');
});

test('a dropped socket reconnects into the game as it now stands', async ({ page }) => {
	const room = await fakeRoom(page, { you: 'X', players: { X: HOST, O: JOINER }, moves: [] });
	await page.goto(`/r/${ROOM}`);
	await expect(board(page)).toBeVisible();
	room.close(1011);
	await expect(page.locator('[data-trouble]')).toHaveText('Connection lost. Reconnecting…');
	await expect(page.locator('[data-spoken]')).toHaveText('Connection lost. Reconnecting…');
	await expect.poll(() => room.hellos()).toBe(2);
	await expect(page.locator('[data-trouble]')).toHaveCount(0);
	await expect(marked(page)).toHaveCount(0);

	room.moved(0, top[0]);
	await expect(marked(page)).toHaveCount(1);
	room.say({ type: 'Moved', ply: 5, cell: top[5] });
	await expect.poll(() => room.hellos(), 'a gap resyncs').toBe(3);
	room.say({ type: 'Rejected', reason: { OutOfOrder: { want: 1 } } });
	await expect.poll(() => room.hellos(), 'a refusal resyncs').toBe(4);
});

test('a Room that isn’t there, or is full, says so', async ({ page }) => {
	await page.goto('/r/abc');
	await expect(page.getByRole('heading', { name: 'This game doesn’t exist' })).toBeVisible();
	await expect(page).toHaveTitle('This game doesn’t exist · Kokonoe');

	const room = await fakeRoom(page, {
		you: 'O',
		players: { X: HOST, O: 'guest-0007' },
		moves: [],
		card: {
			host: HOST,
			variant: 'Standard',
			rules: 'std',
			players: { X: HOST, O: 'guest-0007' },
			you: null,
			takebacks: false
		}
	});
	await page.goto(`/r/${ROOM}`);
	await expect(page.getByRole('heading', { name: 'This game is full' })).toBeVisible();
	await expect(page.getByText(`${HOST} and guest-0007 are playing it.`)).toBeVisible();
	expect(room.hellos()).toBe(0);
});

test('the server’s closes end the game as they should', async ({ page }) => {
	const room = await fakeRoom(page, { you: 'X', players: { X: HOST, O: JOINER }, moves: [] });
	await page.goto(`/r/${ROOM}`);
	await expect(board(page)).toBeVisible();
	room.close(4004);
	await expect(page.getByRole('heading', { name: 'This game doesn’t exist' })).toBeVisible();

	await page.goto(`/r/${ROOM}`);
	await expect(board(page)).toBeVisible();
	const reloaded = page.waitForEvent('load');
	room.close(4001);
	await reloaded;
	await expect(board(page)).toBeVisible();
});

test('a whole game ends in the banner, with no new game to start', async ({ page }) => {
	const room = await fakeRoom(page, {
		you: 'O',
		players: { X: HOST, O: JOINER },
		moves: top.slice(0, -1)
	});
	await page.goto(`/r/${ROOM}`);
	await expect(marked(page)).toHaveCount(top.length - 1);
	room.moved(top.length - 1, top[top.length - 1]);
	room.say({ type: 'Ended', result: '1-0', termination: 'line' });
	await expect(page.getByRole('heading', { name: 'X wins on a line' })).toBeVisible();
	await expect(page.getByRole('button', { name: 'New game' })).toHaveCount(0);
	expect(room.hellos(), 'the end agreed with the engine').toBe(1);
});

test.describe('a Room’s game kept on this device', () => {
	test('a finished one shows in Recent games, and opens its Replay', async ({ page }) => {
		const room = await fakeRoom(page, {
			you: 'O',
			players: { X: HOST, O: JOINER },
			moves: top.slice(0, -1)
		});
		await page.goto(`/r/${ROOM}`);
		await expect(marked(page)).toHaveCount(top.length - 1);
		room.moved(top.length - 1, top[top.length - 1]);
		await expect(page.getByRole('link', { name: 'Replay' })).toBeVisible();

		await page.goto('/');
		const recent = page.getByRole('link', {
			name: new RegExp(`vs ${HOST} ?, Standard ?, X wins on a line · Replay$`)
		});
		await recent.click();
		await expect(page).toHaveURL(`/replay?game=${ROOM}`);
		await expect(page.locator('[data-caption]')).toBeVisible();
	});

	test('an unfinished one continues at its Room, from Home or from /play', async ({ page }) => {
		const room = await fakeRoom(page, { you: 'X', players: { X: HOST, O: JOINER }, moves: [] });
		await page.goto(`/r/${ROOM}`);
		await expect(board(page)).toBeVisible();
		room.moved(0, top[0]);
		await expect(marked(page)).toHaveCount(1);

		await page.goto('/');
		await expect(page.getByText(`${JOINER}’s move`)).toBeVisible();
		await expect(page.getByText(`Standard · with ${JOINER} · 1 move`)).toBeVisible();
		const listed = page.getByRole('link', {
			name: new RegExp(`vs ${JOINER} ?, Standard ?, Unfinished · Resume$`)
		});
		await expect(listed).toHaveAttribute('href', `/r/${ROOM}`);
		await page.getByRole('link', { name: new RegExp(`^Resume ?, vs ${JOINER}$`) }).click();
		await expect(page).toHaveURL(`/r/${ROOM}`);
		await expect(marked(page)).toHaveCount(1);

		await page.goto(`/play?game=${ROOM}`);
		await expect(page).toHaveURL(`/r/${ROOM}`);
		await expect(board(page)).toBeVisible();
	});

	test('offline, its link shows the game as it was last seen', async ({ page }) => {
		const room = await fakeRoom(page, { you: 'X', players: { X: HOST, O: JOINER }, moves: [] });
		await page.goto(`/r/${ROOM}`);
		await expect(board(page)).toBeVisible();
		room.moved(0, top[0]);
		await expect(marked(page)).toHaveCount(1);

		await page.unrouteAll();
		await page.goto(`/r/${ROOM}`);
		await expect(page.getByRole('heading', { name: 'You’re offline' })).toBeVisible();
		await expect(page.getByRole('figure', { name: `${JOINER}’s move` })).toBeVisible();
		await expect(
			page.getByRole('img', { name: 'The board as you last saw it, after 1 move' })
		).toBeVisible();
	});
});
