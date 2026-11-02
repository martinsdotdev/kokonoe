import { expect, test, type Page } from '@playwright/test';
import { TOP_ROW, cells } from '../lib/game/games.fixtures.js';
import { HOST, JOINER, NEXT, ROOM, fakeRoom } from './room.fixtures.js';
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
	await page.getByRole('button', { name: 'Create invite Room' }).click();

	await expect(page).toHaveURL(`/r/${ROOM}`);
	expect(room.created).toEqual([{ rules: 'std', seat: 'Random', takebacks: false }]);
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
			takebacks: false
		}
	});
	await page.goto(`/r/${ROOM}`);
	await expect(
		page.getByRole('heading', { name: `${HOST} invites you to a game of Standard` })
	).toBeVisible();
	await expect(page).toHaveTitle(`${HOST} invites you · Kokonoe`);
	const chips = page.getByRole('list', { name: 'The game' }).getByRole('listitem');
	await expect(chips).toHaveText(['Preset', 'Casual', 'Untimed', 'No takebacks']);
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
				takebacks: true
			}
		});
		await page.goto('/new-game?vs=friend');
		await page.getByRole('checkbox', { name: 'Allow takebacks' }).check();
		await expect(page.getByText('Allowed, if the other player agrees')).toBeVisible();
		await page.getByRole('button', { name: 'Create invite Room' }).click();
		await expect
			.poll(() => room.created)
			.toEqual([{ rules: 'std', seat: 'Random', takebacks: true }]);
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
		room.say({ type: 'TookBack', to_ply: 1 });
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
