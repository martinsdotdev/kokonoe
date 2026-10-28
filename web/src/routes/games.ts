/** Games the end-to-end tests play, as engine-test digit pairs: `"42"` is Space 4, then Space 2. */

/** X takes Local boards 0, 1 and 2, each with a row, while O sends X wherever it needs to go. */
export const TOP_ROW = '03 30 04 40 05 51 13 31 14 41 15 52 26 62 27 72 28';

/** The shortest drawn game among 200,000 random ones, from the engine's tests. */
export const DRAWN =
	'82 21 12 27 72 24 46 64 42 78 85 54 44 11 16 68 87 70 03 32 65 56 ' +
	'62 01 14 67 77 71 38 83 36 63 34 07 76 66 84 04 37 74 80 52 81';

/** Each move's Local board and its Cell's Space within it. */
export const parse = (moves: string) =>
	moves
		.split(/\s+/)
		.filter(Boolean)
		.map((digits) => ({ space: Number(digits[0]), inner: Number(digits[1]) }));

/** A digit pair as Notation: `"42"` is Cell 5·3. */
export const notation = (digits: string) => [...digits].map((digit) => Number(digit) + 1).join('·');
