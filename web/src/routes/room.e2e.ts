import { expect, test, type Page } from '@playwright/test';
import { TOP_ROW, cells } from '../lib/game/games.fixtures.js';
import { HOST, JOINER, ROOM, fakeRoom } from './room.fixtures.js';
import { cell, marked } from './steps.js';

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
	await expect(page.getByRole('button', { name: 'Copied' })).toBeVisible();
	expect(await page.evaluate(() => navigator.clipboard.readText())).toBe(link);

	room.say({
		type: 'Welcome',
		you: 'X',
		snapshot: {
			record: {
				rules_version: 1,
				ruleset: {
					depth: 2,
					continuation: 'Off',
					drawn_owner: 'Neither',
					objective: 'Line',
					fallback: 'Draw',
					opening: 'Free'
				},
				seed: '00000000005eed00',
				moves: []
			},
			players: { X: HOST, O: JOINER }
		}
	});
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
			you: null
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
	expect(room.said[0]).toEqual({
		type: 'Hello',
		protocol: 1,
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
	await expect(page.getByRole('alert')).toHaveText('Connection lost. Reconnecting…');
	await expect.poll(() => room.hellos()).toBe(2);
	await expect(page.getByRole('alert')).toHaveCount(0);
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
			you: null
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
	room.say({ type: 'Ended', outcome: { Objective: 'X' } });
	await expect(page.getByRole('heading', { name: 'X wins on a line' })).toBeVisible();
	await expect(page.getByRole('button', { name: 'New game' })).toHaveCount(0);
	expect(room.hellos(), 'the end agreed with the engine').toBe(1);
});
