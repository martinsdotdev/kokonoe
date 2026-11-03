import { describe, expect, it } from 'vitest';
import { clockText, clockWords, elapsedText, left, type ClockView } from './clock.js';

describe('a clock in words', () => {
	it('writes minutes and seconds, rounded down, and tenths under 10 seconds', () => {
		const cases: [number, string][] = [
			[905_000, '15:05'],
			[180_000, '3:00'],
			[179_999, '2:59'],
			[60_000, '1:00'],
			[10_000, '0:10'],
			[9_999, '0:09.9'],
			[1_250, '0:01.2'],
			[99, '0:00.0'],
			[0, '0:00.0']
		];
		for (const [ms, text] of cases) expect(clockText(ms), `${ms} ms`).toBe(text);
	});

	it('says it in words for screen readers, whole seconds rounded down', () => {
		const cases: [number, string][] = [
			[905_000, '15 minutes 5 seconds'],
			[180_000, '3 minutes'],
			[61_999, '1 minute 1 second'],
			[9_900, '9 seconds'],
			[999, '0 seconds'],
			[3_723_000, '1 hour 2 minutes 3 seconds'],
			[7_200_000, '2 hours']
		];
		for (const [ms, words] of cases) expect(clockWords(ms), `${ms} ms`).toBe(words);
	});
});

describe('how long a player has been away', () => {
	it('writes whole seconds as m:ss, and from an hour as h:mm:ss', () => {
		const cases: [number, string][] = [
			[0, '0:00'],
			[999, '0:00'],
			[65_000, '1:05'],
			[3_599_999, '59:59'],
			[3_600_000, '1:00:00'],
			[86_400_000, '24:00:00']
		];
		for (const [ms, text] of cases) expect(elapsedText(ms), `${ms} ms`).toBe(text);
	});
});

describe('the time left', () => {
	const clocks: ClockView = { ms: { X: 5_000, O: 90_000 }, running: 'X', at: 1_000, aborts: null };

	it('counts down only the clock that runs, to 0', () => {
		expect(left(clocks, 'X', 3_500)).toBe(2_500);
		expect(left(clocks, 'O', 3_500)).toBe(90_000);
		expect(left(clocks, 'X', 60_000)).toBe(0);
	});

	it('never counts up before the moment the clocks were read', () => {
		expect(left(clocks, 'X', 500)).toBe(5_000);
	});

	it('holds a clock that doesn’t run', () => {
		expect(left({ ...clocks, running: null }, 'X', 60_000)).toBe(5_000);
	});
});
