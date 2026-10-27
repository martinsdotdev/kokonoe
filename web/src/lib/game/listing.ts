/** How Home words a saved game: when it was played, whose move it is, and what it is. */
import type { SavedGame } from './saved.js';

const time = new Intl.DateTimeFormat('en-GB', { hour: '2-digit', minute: '2-digit' });
const day = new Intl.DateTimeFormat('en-GB', { day: 'numeric', month: 'short' });
const dayOfYear = new Intl.DateTimeFormat('en-GB', {
	day: 'numeric',
	month: 'short',
	year: 'numeric'
});

const sameDay = (a: Date, b: Date) => a.toDateString() === b.toDateString();

/** "Today, 14:05", "Yesterday, 09:12", "27 Oct", or "27 Oct 2025" from another year. */
export const playedWhen = (updated: number, now = Date.now()): string => {
	const then = new Date(updated);
	const today = new Date(now);
	const yesterday = new Date(now);
	yesterday.setDate(today.getDate() - 1);
	if (sameDay(then, today)) return `Today, ${time.format(then)}`;
	if (sameDay(then, yesterday)) return `Yesterday, ${time.format(then)}`;
	return (then.getFullYear() === today.getFullYear() ? day : dayOfYear).format(then);
};

/** "1 move" or "12 moves": the moves players made, as the result banner counts them. */
export const moves = (n: number): string => `${n} ${n === 1 ? 'move' : 'moves'}`;

/** Whose move an unfinished game waits for: "Your move against the computer". */
export const turnLine = ({ opponent, toMove }: SavedGame): string => {
	if (opponent.kind === 'hotseat') return `${toMove} to move · pass and play`;
	return toMove === opponent.seat ? 'Your move against the computer' : 'The computer’s move';
};

/** "Standard · Level 3 · 12 moves", or "Tic-Tac-Ku · 4 moves" when passing and playing. */
export const detailLine = ({ variant, opponent, record }: SavedGame): string =>
	[
		variant.name,
		opponent.kind === 'computer' ? `Level ${opponent.level}` : null,
		moves(record.moves.length)
	]
		.filter(Boolean)
		.join(' · ');
