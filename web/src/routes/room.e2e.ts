import { expect, test, type Page } from '@playwright/test';
import { TOP_ROW, cells } from '../lib/game/games.fixtures.js';
import {
	HOST,
	JOINER,
	NEXT,
	ROOM,
	awayNow,
	clocksNow,
	fakeRoom,
	keptGame,
	type FakeRoom
} from './room.fixtures.js';
import {
	cell,
	focusBoard,
	marked,
	settledFocus,
	stepThrough,
	turnLine,
	typeMoves
} from './steps.js';

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
	await expect(page.getByRole('checkbox', { name: 'Allow takebacks' })).not.toBeChecked();
	await expect(page.getByRole('radio', { name: 'Untimed' })).toBeChecked();
	await page.getByRole('button', { name: 'Create invite Room' }).click();

	await expect(page).toHaveURL(`/r/${ROOM}`);
	expect(room.created).toEqual([
		{ rules: 'std', seat: 'Random', takebacks: false, clock: null, spectators: false }
	]);
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

	test('an offer shows on the screen, above the players', async ({ page }) => {
		const room = await fakeRoom(page, {
			you: 'X',
			players: { X: HOST, O: JOINER },
			moves: top.slice(0, 2),
			snapshot: { takebacks: true }
		});
		await page.goto(`/r/${ROOM}`);
		await expect(marked(page)).not.toHaveCount(0);
		room.say({ type: 'Offer', kind: 'Takeback', by: 'O', lapses_in: 15_000 });
		await expect(page.getByRole('button', { name: 'Accept' })).toBeInViewport();
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
			takebacks: false,
			clock: null,
			spectators: false
		}
	});
	await page.goto(`/r/${ROOM}`);
	await expect(
		page.getByRole('heading', { name: `${HOST} invites you to a game of Standard` })
	).toBeVisible();
	await expect(page).toHaveTitle(`${HOST} invites you · Kokonoe`);
	const chips = page.getByRole('list', { name: 'The game' }).getByRole('listitem');
	await expect(chips).toHaveText(['Preset', 'Casual', 'Untimed', 'No takebacks', 'No spectators']);
	expect(room.hellos(), 'no socket before Accept').toBe(0);
	await page.getByRole('link', { name: 'Decline' }).click();
	await expect(page).toHaveURL('/');

	await page.goto(`/r/${ROOM}`);
	await page.getByRole('button', { name: 'Accept' }).click();
	await expect(board(page)).toBeVisible();
	await expect(page.locator('main h1'), 'focus follows the page into the game').toBeFocused();
	expect(room.said[0]).toEqual({
		type: 'Hello',
		protocol: 4,
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
	room.say({ type: 'Moved', ply: 5, cell: top[5], clocks: null });
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
			takebacks: false,
			clock: null,
			spectators: false
		}
	});
	await page.goto(`/r/${ROOM}`);
	await expect(page.getByRole('heading', { name: 'This game is full' })).toBeVisible();
	await expect(
		page.getByText(`${HOST} and guest-0007 are playing it, and its host didn’t let anyone watch.`)
	).toBeVisible();
	expect(room.hellos()).toBe(0);
});

test('the server’s closes end the game as they should', async ({ page }) => {
	const room = await fakeRoom(page, { you: 'X', players: { X: HOST, O: JOINER }, moves: [] });
	await page.goto(`/r/${ROOM}`);
	await expect(board(page)).toBeVisible();
	room.close(4004);
	// Only an abort deletes a Room, so one gone while the page played it was aborted.
	await expect(page.getByRole('heading', { name: 'Game aborted' })).toBeVisible();
	await expect(page.getByText(/^A player didn’t make a first move/)).toBeVisible();

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

test.describe('a Room’s endings', () => {
	const spoken = (page: Page) => page.locator('[data-spoken]');
	const offer = (page: Page) => page.getByRole('region', { name: 'Offer' });

	test('a host allows takebacks from the sheet, and a joiner’s card says so', async ({ page }) => {
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
				takebacks: true,
				clock: null,
				spectators: false
			}
		});
		await page.goto('/new-game?vs=friend');
		await page.getByRole('checkbox', { name: 'Allow takebacks' }).check();
		await expect(page.getByText('Allowed, if the other player agrees')).toBeVisible();
		await page.getByRole('button', { name: 'Create invite Room' }).click();
		await expect
			.poll(() => room.created)
			.toEqual([{ rules: 'std', seat: 'Random', takebacks: true, clock: null, spectators: false }]);
		await expect(page.getByText('Takebacks allowed')).toBeVisible();
	});

	test('resigning asks first, then the banner says who resigned', async ({ page }) => {
		const room = await fakeRoom(page, {
			you: 'X',
			players: { X: HOST, O: JOINER },
			moves: top.slice(0, 2)
		});
		await page.goto(`/r/${ROOM}`);
		await page.getByRole('button', { name: 'Resign' }).click();
		await page.getByRole('button', { name: 'Yes, resign' }).click();
		await expect.poll(() => room.said.at(-1)).toEqual({ type: 'Resign' });
		room.say({ type: 'Ended', result: '0-1', termination: 'resign' });
		await expect(page.getByRole('heading', { name: 'X resigned. O wins.' })).toBeVisible();
		await expect(spoken(page)).toHaveText('X resigned. O wins.');
		await expect(page.getByRole('button', { name: 'Rematch' })).toBeEnabled();
	});

	test('a draw offered is shown at once, and ends the game once accepted', async ({ page }) => {
		const room = await fakeRoom(page, {
			you: 'X',
			players: { X: HOST, O: JOINER },
			moves: top.slice(0, 2)
		});
		await page.goto(`/r/${ROOM}`);
		await page.getByRole('button', { name: 'Offer draw' }).click();
		await expect.poll(() => room.said.at(-1)).toEqual({ type: 'OfferDraw' });
		await expect(offer(page)).toHaveText(`You offered a draw. Waiting for ${JOINER}…`);
		await expect(page.getByRole('button', { name: 'Offer draw' })).toBeDisabled();
		room.say({ type: 'Offer', kind: 'Draw', by: 'X', lapses_in: null });
		room.say({ type: 'OfferAnswered', kind: 'Draw', accepted: true });
		room.say({ type: 'Ended', result: '1/2-1/2', termination: 'agreed draw' });
		await expect(page.getByRole('heading', { name: 'Draw agreed' })).toBeVisible();
		await expect(spoken(page)).toHaveText('Draw agreed.');
		await expect(offer(page)).toHaveCount(0);
	});

	test('the other player’s draw is said once, and declined', async ({ page }) => {
		const room = await fakeRoom(page, {
			you: 'X',
			players: { X: HOST, O: JOINER },
			moves: top.slice(0, 2)
		});
		await page.goto(`/r/${ROOM}`);
		await expect(marked(page)).toHaveCount(2);
		room.say({ type: 'Offer', kind: 'Draw', by: 'O', lapses_in: null });
		await expect(offer(page)).toContainText(`${JOINER} offers a draw.`);
		await expect(spoken(page)).toContainText(`${JOINER} offers a draw.`);
		// Kit's route announcer lives outside the page's content.
		const live = page.locator('main :is([aria-live], [role="status"], [role="alert"])');
		await expect(live, 'the game’s one live line').toHaveCount(1);
		await offer(page).getByRole('button', { name: 'Decline' }).focus();
		await page.keyboard.press('Enter');
		await expect.poll(() => room.said.at(-1)).toEqual({ type: 'AnswerDraw', accept: false });
		await expect(offer(page)).toHaveCount(0);
		await expect(spoken(page)).not.toContainText('offers a draw');
		await expect(page.locator('main h1'), 'focus goes to the game with the card').toBeFocused();
	});

	test('an offer waiting survives a reconnect, counting down from the server’s word', async ({
		page
	}) => {
		const state = {
			you: 'X' as const,
			players: { X: HOST, O: JOINER },
			moves: top.slice(0, 2),
			snapshot: {}
		};
		const room = await fakeRoom(page, state);
		await page.goto(`/r/${ROOM}`);
		await expect(marked(page)).toHaveCount(2);
		room.say({ type: 'Offer', kind: 'Takeback', by: 'O', lapses_in: 15_000 });
		await expect(offer(page)).toBeVisible();
		state.snapshot = { offer: { kind: 'Takeback', by: 'O', lapses_in: 9000 } };
		room.close(1011);
		await expect.poll(() => room.hellos()).toBe(2);
		await expect(page.locator('[data-trouble]')).toHaveCount(0);
		await expect(offer(page)).toContainText(`${JOINER} asks to take back their last move.`);
		await expect(offer(page).getByRole('button', { name: 'Accept' })).toBeVisible();
		await expect(offer(page).locator('[data-left]')).toHaveText(/^[1-9] seconds? left to answer$/);
	});

	test('a takeback that lapses says so, and can be asked again', async ({ page }) => {
		const room = await fakeRoom(page, {
			you: 'O',
			players: { X: HOST, O: JOINER },
			moves: top.slice(0, 2),
			snapshot: { takebacks: true }
		});
		await page.goto(`/r/${ROOM}`);
		const ask = page.getByRole('button', { name: 'Ask takeback' });
		await ask.click();
		room.say({ type: 'Offer', kind: 'Takeback', by: 'O', lapses_in: 15_000 });
		await expect(ask).toHaveAttribute('aria-disabled', 'true');
		await expect(ask, 'a disabled control keeps focus').toBeFocused();
		room.say({ type: 'OfferAnswered', kind: 'Takeback', accepted: false });
		await expect(page.locator('[data-notice]')).toHaveText(`${HOST} didn’t accept the takeback.`);
		await expect(ask).toHaveAttribute('aria-disabled', 'false');
	});

	test('a takeback asked counts down, and takes the move back once accepted', async ({ page }) => {
		const state = {
			you: 'O' as const,
			players: { X: HOST, O: JOINER },
			moves: top.slice(0, 2),
			snapshot: { takebacks: true }
		};
		const room = await fakeRoom(page, state);
		await page.goto(`/r/${ROOM}`);
		await expect(marked(page)).toHaveCount(2);
		await page.getByRole('button', { name: 'Ask takeback' }).click();
		await expect.poll(() => room.said.at(-1)).toEqual({ type: 'AskTakeback' });
		room.say({ type: 'Offer', kind: 'Takeback', by: 'O', lapses_in: 15_000 });
		await expect(offer(page).locator('[data-left]')).toHaveText(/^\d+ seconds left to answer$/);
		room.say({ type: 'OfferAnswered', kind: 'Takeback', accepted: true });
		state.moves = top.slice(0, 1);
		room.say({ type: 'TookBack', to_ply: 1, clocks: null });
		await expect(marked(page)).toHaveCount(1);
		await expect(offer(page)).toHaveCount(0);
		await expect(turnLine(page, 'O: play in board 4.')).toBeVisible();
	});

	test('a rematch accepted takes both pages to the new Room', async ({ page }) => {
		const state = { you: 'O' as const, players: { X: HOST, O: JOINER }, moves: top };
		const room = await fakeRoom(page, state);
		await page.goto(`/r/${ROOM}`);
		await expect(page.getByRole('heading', { name: 'X wins on a line' })).toBeVisible();
		await page.getByRole('button', { name: 'Rematch' }).click();
		await expect.poll(() => room.said.at(-1)).toEqual({ type: 'OfferRematch' });
		await expect(offer(page)).toHaveText(`You offered a rematch. Waiting for ${HOST}…`);
		room.say({ type: 'Offer', kind: 'Rematch', by: 'O', lapses_in: 60_000 });
		room.say({ type: 'OfferAnswered', kind: 'Rematch', accepted: true });
		Object.assign(state, { you: 'X', players: { X: JOINER, O: HOST }, moves: [] });
		room.say({ type: 'RematchRoom', room: NEXT });
		await expect(page).toHaveURL(`/r/${NEXT}`);
		await expect(board(page)).toBeVisible();
		await expect(turnLine(page, 'X: play anywhere.')).toBeVisible();
		expect(room.said.filter((message) => message.type === 'Hello').at(-1)).toMatchObject({
			room: NEXT
		});
	});

	test('a rematch declined says so, and can be offered again', async ({ page }) => {
		const room = await fakeRoom(page, { you: 'O', players: { X: HOST, O: JOINER }, moves: top });
		await page.goto(`/r/${ROOM}`);
		await page.getByRole('button', { name: 'Rematch' }).click();
		room.say({ type: 'Offer', kind: 'Rematch', by: 'O', lapses_in: 60_000 });
		room.say({ type: 'OfferAnswered', kind: 'Rematch', accepted: false });
		await expect(page.locator('[data-notice]')).toHaveText(`${HOST} didn’t accept the rematch.`);
		await expect(spoken(page)).toContainText(`${HOST} didn’t accept the rematch.`);
		await expect(page.getByRole('button', { name: 'Rematch' })).toBeEnabled();
	});

	test('a finished Room whose rematch was made links to it', async ({ page }) => {
		await fakeRoom(page, {
			you: 'O',
			players: { X: HOST, O: JOINER },
			moves: top,
			snapshot: { rematch: NEXT }
		});
		await page.goto(`/r/${ROOM}`);
		await expect(page.getByRole('link', { name: 'Play the rematch' })).toHaveAttribute(
			'href',
			`/r/${NEXT}`
		);
		await expect(page.getByRole('button', { name: 'Rematch' })).toHaveCount(0);
	});
});

test.describe('a timed Room', () => {
	const players = { X: HOST, O: JOINER };
	const clock = (page: Page, player: 'X' | 'O') =>
		page.locator('[data-clock]').nth(player === 'X' ? 0 : 1);
	const spoken = (page: Page) => page.locator('[data-spoken]');

	test('a host picks a clock from the sheet, and a joiner’s card names it', async ({ page }) => {
		const room = await fakeRoom(page, { you: 'X', players: { X: HOST, O: null }, moves: [] });
		await page.goto('/new-game?vs=friend');
		await page.getByRole('radio', { name: '5+3' }).check();
		await expect(
			page.getByText('Casual, 5+3: 5 minutes each, plus 3 seconds a move')
		).toBeVisible();
		await page.getByRole('button', { name: 'Create invite Room' }).click();
		await expect(page).toHaveURL(`/r/${ROOM}`);
		expect(room.created).toEqual([
			{ rules: 'std', seat: 'Random', takebacks: false, clock: '5+3', spectators: false }
		]);

		await page.unrouteAll();
		await fakeRoom(page, {
			you: 'O',
			players,
			moves: [],
			card: {
				host: HOST,
				variant: 'Standard',
				rules: 'std',
				players: { X: HOST, O: null },
				you: null,
				takebacks: false,
				clock: '3+2',
				spectators: false
			}
		});
		await page.goto(`/r/${ROOM}`);
		const chips = page.getByRole('list', { name: 'The game' }).getByRole('listitem');
		await expect(chips).toHaveText([
			'Preset',
			'Casual',
			'3+2 clock',
			'No takebacks',
			'No spectators'
		]);
	});

	test('the clocks wait for each first move, then run, and show time running low', async ({
		page
	}) => {
		const room = await fakeRoom(page, {
			you: 'X',
			players,
			moves: [],
			snapshot: { clock: '3+2', clocks: clocksNow({ aborts_at: Date.now() + 30_000 }) }
		});
		await page.goto(`/r/${ROOM}`);
		await expect(clock(page, 'X')).toHaveText('3:00');
		await expect(clock(page, 'O')).toHaveText('3:00');
		await expect(
			page.getByText(/^Make your first move within \d+ seconds, or the game is aborted\.$/)
		).toBeVisible();
		await expect(spoken(page)).toHaveText(
			'X: play anywhere. You have 30 seconds to make your first move.'
		);

		await cell(page, digits[0]).click();
		room.moved(0, top[0], clocksNow({ aborts_at: Date.now() + 30_000 }));
		await expect(
			page.getByText(new RegExp(`^${JOINER} has \\d+ seconds to make a first move`))
		).toBeVisible();

		room.moved(1, top[1], clocksNow({ x_ms: 10_400, running: 'X' }));
		await expect(clock(page, 'X')).toHaveText(/^0:0\d\.\d$/);
		await expect(clock(page, 'X')).toHaveAttribute('data-low');
		await expect(clock(page, 'O')).not.toHaveAttribute('data-low');
		await expect(spoken(page)).toHaveText('10 seconds left.');
		await expect(page.getByText(/to make a first move/)).toHaveCount(0);
	});

	test('a game lost on time says so, its clock run out', async ({ page }) => {
		const room = await fakeRoom(page, {
			you: 'X',
			players,
			moves: top.slice(0, 2),
			snapshot: { clock: '3+2', clocks: clocksNow({ x_ms: 4_000, running: 'X' }) }
		});
		await page.goto(`/r/${ROOM}`);
		await expect(clock(page, 'X')).toHaveText(/^0:0[0-3]\.\d$/);
		room.say({ type: 'Ended', result: '0-1', termination: 'timeout' });
		await expect(page.getByRole('heading', { name: 'X ran out of time. O wins.' })).toBeVisible();
		await expect(clock(page, 'X')).toHaveText('0:00.0');
		await expect(page.locator('[data-to-move]'), 'no turn once the game stops').toHaveCount(0);
		expect(room.hellos(), 'the end agreed with the engine').toBe(1);
	});

	test('a reconnect shows the server’s clocks', async ({ page }) => {
		const state = {
			you: 'X' as const,
			players,
			moves: top.slice(0, 3),
			snapshot: { clock: '3+2' as const, clocks: clocksNow({ running: 'O' }) }
		};
		const room = await fakeRoom(page, state);
		await page.goto(`/r/${ROOM}`);
		await expect(clock(page, 'O')).toHaveText(/^2:5\d$/);
		state.snapshot.clocks = clocksNow({ o_ms: 95_000, x_ms: 171_000 });
		room.close(1006);
		await expect(clock(page, 'O')).toHaveText('1:35');
		await expect(clock(page, 'X')).toHaveText('2:51');
	});

	test('an aborted game says so, and Home stops offering it', async ({ page }) => {
		const snapshot = {
			clock: '3+2' as const,
			clocks: clocksNow({ aborts_at: Date.now() + 30_000 })
		};
		/** Whether this page still keeps the Room's game. */
		const kept = async () => (await keptGame(page, ROOM)) !== undefined;
		const room = await fakeRoom(page, { you: 'X', players, moves: [], snapshot });
		await page.goto(`/r/${ROOM}`);
		await expect(board(page)).toBeVisible();
		room.moved(0, top[0], clocksNow({ aborts_at: Date.now() + 30_000 }));
		await expect(marked(page)).toHaveCount(1);
		await expect.poll(kept).toBe(true);
		await page.goto('/');
		const resume = page.getByRole('link', { name: new RegExp(`^Resume ?, vs ${JOINER}$`) });
		await expect(resume).toBeVisible();

		await resume.click();
		await expect(marked(page)).toHaveCount(1);
		room.say({ type: 'Aborted', reason: 'FirstMoveTimeout' });
		await expect(page.getByRole('heading', { name: 'Game aborted' })).toBeVisible();
		await expect(
			page.getByText(`${JOINER} didn’t make a first move in time, so the game wasn’t kept.`)
		).toBeVisible();
		await expect.poll(kept, 'this page’s copy is deleted').toBe(false);
		room.close(1011);
		await page.waitForTimeout(500);
		expect(room.hellos(), 'the server’s close isn’t a reason to reconnect').toBe(2);

		await page.getByRole('link', { name: 'Go to Home' }).click();
		await expect(page.getByRole('heading', { name: 'Invite a friend' })).toBeVisible();
		await expect(resume).toHaveCount(0);
	});
});

test.describe('a player away', () => {
	const players = { X: HOST, O: JOINER };
	const spoken = (page: Page) => page.locator('[data-spoken]');
	const away = (page: Page) => page.locator('[data-away]');
	const card = (page: Page) => page.getByRole('region', { name: 'Claim' });

	test('the other player leaving shows in their bar, and their coming back', async ({ page }) => {
		const room = await fakeRoom(page, { you: 'X', players, moves: top.slice(0, 2) });
		await page.goto(`/r/${ROOM}`);
		await expect(board(page)).toBeVisible();
		room.say({ type: 'Presence', player: 'O', away: awayNow() });
		await expect(away(page)).toHaveText(/^Away 0:0\d$/);
		await expect(spoken(page)).toHaveText(new RegExp(`${JOINER} left the game\\.$`));
		await expect(page.locator('[data-notice]')).toHaveText(`${JOINER} left the game.`);
		await expect(card(page), 'not before the Claim is due').toHaveCount(0);

		room.say({ type: 'Presence', player: 'O', away: null });
		await expect(away(page)).toHaveCount(0);
		await expect(spoken(page)).toHaveText(new RegExp(`${JOINER} is back\\.$`));
	});

	test('a Claim comes due, and claims the win', async ({ page }) => {
		const room = await fakeRoom(page, { you: 'X', players, moves: top.slice(0, 2) });
		await page.goto(`/r/${ROOM}`);
		await expect(board(page)).toBeVisible();
		room.say({ type: 'Presence', player: 'O', away: awayNow(0, 1_000) });
		await expect(card(page)).toBeVisible();
		await expect(spoken(page)).toHaveText(/You can claim the win or a draw, or keep waiting\.$/);
		await expect(page.locator('[data-notice]'), 'the card says it').toHaveCount(0);

		await card(page).getByRole('button', { name: 'Claim the win' }).click();
		await expect.poll(() => room.said.at(-1)).toEqual({ type: 'Claim', as: 'Win' });
		room.say({ type: 'Ended', result: '1-0', termination: 'claim' });
		const won = 'O left the game. X claimed the win.';
		await expect(page.getByRole('heading', { name: won })).toBeVisible();
		await expect(spoken(page)).toHaveText(won);
		await expect(card(page)).toHaveCount(0);
		await expect(page.locator('main h1')).toBeFocused();
		await expect(away(page), 'no one is kept waiting once it’s over').toHaveCount(0);
		expect(room.hellos(), 'the end agreed with the engine').toBe(1);
	});

	test('a page that comes back after a while can claim at once, and claims a draw', async ({
		page
	}) => {
		const room = await fakeRoom(page, {
			you: 'O',
			players,
			moves: top.slice(0, 3),
			snapshot: { presence: { X: awayNow(65_000, 0), O: null } }
		});
		await page.goto(`/r/${ROOM}`);
		await expect(away(page)).toHaveText(/^Away 1:0\d$/);
		await card(page).getByRole('button', { name: 'Claim a draw' }).click();
		await expect.poll(() => room.said.at(-1)).toEqual({ type: 'Claim', as: 'Draw' });
		room.say({ type: 'Ended', result: '1/2-1/2', termination: 'claim' });
		await expect(page.getByRole('heading', { name: 'Draw claimed' })).toBeVisible();
		await expect(page.locator('[data-disc]')).toHaveText('=');
	});

	test('a game both players left is abandoned, and Home lists it as finished', async ({ page }) => {
		const state: FakeRoom = { you: 'X', players, moves: top.slice(0, 2) };
		await fakeRoom(page, state);
		await page.goto(`/r/${ROOM}`);
		await expect(marked(page)).toHaveCount(2);
		await expect.poll(async () => (await keptGame(page, ROOM))?.result).toBeNull();
		await page.goto('/');
		const listed = (result: string) =>
			page.getByRole('link', { name: new RegExp(`vs ${JOINER} ?, Standard ?, ${result}$`) });
		await expect(listed('Unfinished · Resume')).toBeVisible();

		state.snapshot = { end: { result: null, termination: 'abandoned' } };
		await page.getByRole('link', { name: new RegExp(`^Resume ?, vs ${JOINER}$`) }).click();
		await expect(page.getByRole('heading', { name: 'Game abandoned' })).toBeVisible();
		await expect(page.locator('[data-disc]')).toHaveText('–');
		await expect(spoken(page)).toHaveText('Game abandoned.');
		await expect.poll(async () => (await keptGame(page, ROOM))?.result).toBe('Game abandoned');

		await page.goto('/');
		await expect(page.getByRole('link', { name: /^Resume/ }), 'nothing to continue').toHaveCount(0);
		await listed('Game abandoned · Replay').click();
		await expect(page).toHaveURL(`/replay?game=${ROOM}`);
		await expect(page.getByRole('heading', { name: 'Game abandoned' })).toBeVisible();
	});
});

test.describe('watching a Room', () => {
	const players = { X: HOST, O: 'guest-0007' };
	/** The card of a Room Spectators may watch, for a page that holds no seat in it. */
	const watchable = (seats: { X: string; O: string | null }) => ({
		host: HOST,
		variant: 'Standard',
		rules: 'std',
		players: seats,
		you: null,
		takebacks: false,
		clock: null,
		spectators: true
	});
	/** The marked Cells of a board that's only to read, as a Spectator's is. */
	const read = (page: Page) => page.getByRole('img', { name: /^Cell \S+, [XO]$/ });

	test('a host lets Spectators watch from the sheet', async ({ page }) => {
		const room = await fakeRoom(page, { you: 'X', players: { X: HOST, O: null }, moves: [] });
		await page.goto('/new-game?vs=friend');
		const allow = page.getByRole('checkbox', { name: 'Allow spectators' });
		await expect(allow).not.toBeChecked();
		await allow.check();
		await expect(page.getByText('Allowed: the players see how many, not who')).toBeVisible();
		await page.getByRole('button', { name: 'Create invite Room' }).click();
		await expect
			.poll(() => room.created)
			.toEqual([{ rules: 'std', seat: 'Random', takebacks: false, clock: null, spectators: true }]);
	});

	test('a full Room’s link watches its game: moves land, and none can be made', async ({
		page
	}) => {
		const room = await fakeRoom(page, {
			you: null,
			players,
			moves: top.slice(0, 2),
			card: watchable(players),
			snapshot: { watching: 1 }
		});
		await page.goto(`/r/${ROOM}`);
		await expect(board(page)).toBeVisible();
		expect(room.said[0]).toEqual({
			type: 'Hello',
			protocol: 4,
			rules_version: 1,
			room: ROOM,
			role: 'Spectator'
		});
		await expect(page).toHaveTitle(`Watching ${HOST} vs guest-0007 · Kokonoe`);
		await expect(page.getByText('You’re watching', { exact: true })).toBeVisible();
		await expect(page.getByRole('link', { name: 'Stop watching' })).toBeVisible();
		await expect(read(page)).toHaveCount(2);
		await expect(page.getByRole('button', { name: /^Cell / })).toHaveCount(0);
		await expect(page.getByRole('button', { name: 'Resign' })).toHaveCount(0);

		room.moved(2, top[2]);
		await expect(read(page)).toHaveCount(3);
		room.say({ type: 'Watching', count: 3 });
		await expect(page.getByText('You’re watching, with 2 others')).toBeVisible();
		room.say({ type: 'Offer', kind: 'Draw', by: 'O', lapses_in: null });
		await expect(page.getByText('guest-0007 offers a draw.', { exact: true })).toBeVisible();
		await expect(page.getByRole('button', { name: 'Accept' })).toHaveCount(0);
		room.say({ type: 'Ended', result: '1-0', termination: 'resign' });
		await expect(page.getByRole('region', { name: 'Result' })).toBeVisible();
		await expect(page.getByRole('button', { name: 'Rematch' })).toHaveCount(0);
		expect(room.said.filter((message) => message.type !== 'Hello')).toEqual([]);

		// The players agree to a rematch, and the Spectator goes to watch it.
		room.say({ type: 'RematchRoom', room: NEXT });
		await expect(page).toHaveURL(`/r/${NEXT}`);
		await expect.poll(() => room.hellos()).toBe(2);
		expect(room.said.at(-1)).toMatchObject({ type: 'Hello', room: NEXT, role: 'Spectator' });
	});

	test('a joiner may watch instead of taking the free seat', async ({ page }) => {
		const seats = { X: HOST, O: null };
		const room = await fakeRoom(page, {
			you: null,
			players: seats,
			moves: [],
			card: watchable(seats)
		});
		await page.goto(`/r/${ROOM}`);
		await expect(page.getByText('Spectators allowed')).toBeVisible();
		await page.getByRole('button', { name: 'Watch' }).click();
		await expect(board(page)).toBeVisible();
		expect(room.said[0]).toMatchObject({ type: 'Hello', role: 'Spectator' });
		await expect(page.locator('[data-unseated]')).toHaveText('Waiting for a second player.');
		await expect(page.getByRole('banner').getByText(`${HOST} vs a free seat`)).toBeVisible();

		room.join('guest-0007');
		await expect(page.locator('[data-unseated]')).toHaveCount(0);
		await expect(turnLine(page, 'X: play anywhere.')).toBeVisible();
	});

	test('a player sees how many watch, a count and never who', async ({ page }) => {
		const room = await fakeRoom(page, { you: 'X', players, moves: [] });
		await page.goto(`/r/${ROOM}`);
		await expect(board(page)).toBeVisible();
		await expect(page.locator('[data-watching]')).toHaveCount(0);
		room.say({ type: 'Watching', count: 2 });
		await expect(page.locator('[data-watching]')).toHaveText('2 watching');
		room.say({ type: 'Watching', count: 0 });
		await expect(page.locator('[data-watching]')).toHaveCount(0);
	});
});
