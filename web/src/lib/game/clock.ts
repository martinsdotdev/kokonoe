/**
 * A timed Room's clocks as the game screen shows them (spec §11): each player's time left, counted
 * down from a moment in `Date.now()`'s time, and written as the player bars write it, as is how
 * long a player has been away. Engine-free.
 */
import type { Player } from '#lib/engine.js';

/**
 * The clocks as they stood at `at`: each player's milliseconds left, whose clock runs from then,
 * and when the game aborts unless the player to move makes their first move. In `Date.now()`'s
 * time.
 */
export interface ClockView {
	readonly ms: Record<Player, number>;
	readonly running: Player | null;
	readonly at: number;
	readonly aborts: number | null;
}

/** Under this, a clock shows it's running low. */
export const LOW_MS = 20_000;

/** The times left the live line says once each, for this page's own clock: highest first. */
export const CUES = [30_000, 10_000] as const;

/** `player`'s milliseconds left at `now`: their clock counts down only while it runs, to 0. */
export const left = (clocks: ClockView, player: Player, now: number): number => {
	const ms = clocks.ms[player];
	return clocks.running === player ? Math.max(0, ms - Math.max(0, now - clocks.at)) : ms;
};

/** A time's whole hours, then minutes and seconds past them, rounded down. */
const parts = (ms: number) => {
	const seconds = Math.floor(ms / 1000);
	return {
		hours: Math.floor(seconds / 3600),
		minutes: Math.floor(seconds / 60) % 60,
		seconds: seconds % 60
	};
};

/**
 * A time in words, for screen readers: "2 minutes 51 seconds", "1 minute", "9 seconds", and for an
 * absence "1 hour 2 minutes 3 seconds".
 */
export const clockWords = (ms: number): string => {
	const { hours, minutes, seconds } = parts(ms);
	const unit = (n: number, word: string) => `${n} ${word}${n === 1 ? '' : 's'}`;
	const words = [];
	if (hours) words.push(unit(hours, 'hour'));
	if (minutes) words.push(unit(minutes, 'minute'));
	if (seconds || !(hours || minutes)) words.push(unit(seconds, 'second'));
	return words.join(' ');
};

/** A time in whole seconds as m:ss, and from an hour as h:mm:ss: 0:05, 1:05, 1:02:03. */
export const elapsedText = (ms: number): string => {
	const { hours, minutes, seconds } = parts(ms);
	const two = (n: number) => String(n).padStart(2, '0');
	return hours ? `${hours}:${two(minutes)}:${two(seconds)}` : `${minutes}:${two(seconds)}`;
};

/** A clock as m:ss, and under 10 s with tenths: 3:00, 2:59, 0:10, 0:09.9, 0:00.0. */
export const clockText = (ms: number): string => {
	if (ms < 10_000) {
		const tenths = Math.floor(ms / 100);
		return `0:0${Math.floor(tenths / 10)}.${tenths % 10}`;
	}
	return elapsedText(ms);
};
