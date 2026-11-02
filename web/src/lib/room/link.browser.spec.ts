import { afterEach, beforeAll, beforeEach, describe, expect, it, vi, type Mock } from 'vitest';
import { CloseCode, loadEngine, type ClientMsg, type ServerMsg } from '#lib/engine.js';
import { Link, type LinkEvents } from './link.js';
import { ROOM, sockets, welcome } from './room.fixtures.js';

const HELLO: ClientMsg = {
	type: 'Hello',
	protocol: 1,
	rules_version: 1,
	room: ROOM,
	role: 'Player'
};

/** A window the test sends `online` and `visibilitychange` from. */
const fakeWindow = () =>
	Object.assign(new EventTarget(), {
		document: { visibilityState: 'hidden' as DocumentVisibilityState }
	});

let heard: ServerMsg[];
let events: LinkEvents & { dropped: Mock<() => void>; ended: Mock<(code: number) => void> };

beforeAll(() => loadEngine());

beforeEach(() => {
	vi.useFakeTimers();
	heard = [];
	events = {
		message: (message) => heard.push(message),
		dropped: vi.fn<() => void>(),
		ended: vi.fn<(code: number) => void>()
	};
});

afterEach(() => vi.useRealTimers());

const link = (options: { delays?: number[] } = {}) => {
	const fake = sockets();
	const window = fakeWindow();
	const made = new Link(HELLO, events, { open: fake.open, random: () => 0.5, window, ...options });
	return { link: made, fake, window };
};

describe('a Room’s link', () => {
	it('says Hello on the socket it opens, and passes on what the server says', () => {
		const { fake } = link();
		expect(fake.last().url).toBe(`ws://${location.host}/api/socket`);
		fake.last().accept();
		expect(fake.last().sent).toEqual([HELLO]);
		fake.last().say(welcome('X'));
		fake.last().say({ type: 'Moved', ply: 0, cell: 40, clocks: null });
		expect(heard.map((message) => message.type)).toEqual(['Welcome', 'Moved']);
	});

	it('sends only while the socket is open', () => {
		const { link: made, fake } = link();
		const move: ClientMsg = { type: 'Move', ply: 0, cell: 40 };
		expect(made.send(move)).toBe(false);
		fake.last().accept();
		expect(made.send(move)).toBe(true);
		expect(fake.last().sent).toEqual([HELLO, move]);
	});

	it('reconnects after a drop, waiting longer each time until a Welcome', () => {
		const { fake } = link({ delays: [100, 400] });
		const waits: number[] = [];
		for (let drop = 0; drop < 3; drop++) {
			fake.last().end(1006);
			const before = fake.opened.length;
			const started = Date.now();
			while (fake.opened.length === before) vi.advanceTimersByTime(10);
			waits.push(Date.now() - started);
		}
		expect(waits).toEqual([100, 400, 400]);
		expect(events.dropped).toHaveBeenCalledTimes(3);

		fake.last().accept();
		fake.last().say(welcome('X'));
		fake.last().end(1006);
		vi.advanceTimersByTime(100);
		expect(fake.opened).toHaveLength(5);
	});

	it('waits for a restarting server', () => {
		const { fake } = link({ delays: [100] });
		fake.last().end(CloseCode.Restarting);
		vi.advanceTimersByTime(999);
		expect(fake.opened).toHaveLength(1);
		vi.advanceTimersByTime(1);
		expect(fake.opened).toHaveLength(2);
	});

	it('tries again at once when the browser is back online or the page in view', () => {
		const { fake, window } = link({ delays: [60_000] });
		fake.last().end(1006);
		window.dispatchEvent(new Event('online'));
		expect(fake.opened).toHaveLength(2);
		fake.last().end(1006);
		window.document.visibilityState = 'visible';
		window.dispatchEvent(new Event('visibilitychange'));
		expect(fake.opened).toHaveLength(3);
	});

	it.each([CloseCode.UpdateRequired, CloseCode.Forbidden, CloseCode.NoRoom])(
		'ends for good on %i',
		(code) => {
			const { fake, window } = link();
			fake.last().end(code);
			vi.advanceTimersByTime(60_000);
			window.dispatchEvent(new Event('online'));
			expect(fake.opened).toHaveLength(1);
			expect(events.ended).toHaveBeenCalledWith(code);
			expect(events.dropped).not.toHaveBeenCalled();
		}
	);

	it('reconnects at once to resync', () => {
		const { link: made, fake } = link();
		fake.last().accept();
		made.resync();
		expect(fake.opened[0].closed).toBe(true);
		expect(fake.opened).toHaveLength(2);
		// The forgotten socket's close says nothing.
		fake.opened[0].end(1006);
		expect(events.dropped).not.toHaveBeenCalled();
	});

	it('backs off before asking again after a frame it can’t read', () => {
		const { fake } = link({ delays: [100] });
		fake.last().accept();
		fake.last().say({ type: 'Resigned' });
		expect(heard).toEqual([]);
		expect(events.dropped).toHaveBeenCalledOnce();
		expect(fake.opened).toHaveLength(1);
		vi.advanceTimersByTime(100);
		expect(fake.opened).toHaveLength(2);
	});

	it('stops when disposed', () => {
		const { link: made, fake, window } = link();
		fake.last().end(1006);
		made.dispose();
		vi.advanceTimersByTime(60_000);
		window.dispatchEvent(new Event('online'));
		expect(fake.opened).toHaveLength(1);
	});
});
