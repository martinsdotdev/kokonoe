import { describe, expect, it } from 'vitest';
import { playQuery, readLaunch, replayQuery, type Launch } from './launch.js';

const read = (query: string) => readLaunch(new URLSearchParams(query));

describe("a game's URL", () => {
	it('defaults to Standard, passed and played', () => {
		expect(read('')).toEqual({ code: 'std', opponent: { kind: 'hotseat' } });
		expect(read('rules=most')).toEqual({ code: 'most', opponent: { kind: 'hotseat' } });
		expect(read('rules=cont+first5+most'), 'a hand-typed +').toEqual({
			code: 'cont+first5+most',
			opponent: { kind: 'hotseat' }
		});
	});

	it('names the computer with a Level and a seat, Level 3 and X by default', () => {
		expect(read('vs=computer')).toEqual({
			code: 'std',
			opponent: { kind: 'computer', level: 3, seat: 'X' }
		});
		expect(read('rules=cont&vs=computer&level=8&seat=O')).toEqual({
			code: 'cont',
			opponent: { kind: 'computer', level: 8, seat: 'O' }
		});
	});

	it('says what is wrong with a bad one', () => {
		expect(read('vs=robot')).toEqual({ error: '“robot” isn’t an opponent.' });
		expect(read('vs=computer&level=9')).toEqual({ error: 'There’s no Level 9.' });
		expect(read('vs=computer&level=2.5')).toEqual({ error: 'There’s no Level 2.5.' });
		expect(read('vs=computer&seat=Z')).toEqual({ error: '“Z” isn’t a seat.' });
	});

	it('round-trips through playQuery', () => {
		const launches: Launch[] = [
			{ code: 'std', opponent: { kind: 'hotseat' } },
			{ code: 'cont+first5+most', opponent: { kind: 'computer', level: 4, seat: 'O' } }
		];
		for (const launch of launches) expect(read(playQuery(launch).slice(1))).toEqual(launch);
		expect(playQuery(launches[1])).toBe('?rules=cont%2Bfirst5%2Bmost&vs=computer&level=4&seat=O');
	});
});

describe("a replay's URL", () => {
	it('names the game, and the ply past the start', () => {
		expect(replayQuery('a1')).toBe('?game=a1');
		expect(replayQuery('a1', 0)).toBe('?game=a1');
		expect(replayQuery('a1', 14)).toBe('?game=a1&ply=14');
		expect(replayQuery(null), 'an imported game, at its start').toBe('');
		expect(replayQuery(null, 3)).toBe('?ply=3');
	});
});
