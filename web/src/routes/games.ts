/**
 * Games the tests play, as the engine's tests write them: each move its Spaces as digits, so
 * `"42"` is Space 4, then Space 2, and at depth 3 `"407"` is Space 4, then 0, then 7.
 */

/** X takes Local boards 0, 1 and 2, each with a row, while O sends X wherever it needs to go. */
export const TOP_ROW = '03 30 04 40 05 51 13 31 14 41 15 52 26 62 27 72 28';

/** The shortest drawn game among 200,000 random ones, from the engine's tests. */
export const DRAWN =
	'82 21 12 27 72 24 46 64 42 78 85 54 44 11 16 68 87 70 03 32 65 56 ' +
	'62 01 14 67 77 71 38 83 36 63 34 07 76 66 84 04 37 74 80 52 81';

/**
 * Deep: X wins Local board 7·6 (8·7 in Notation), then O's 0·7·6 Sends X into it, so X may play
 * anywhere in middle Board 7. `INTO_A_WON_BOARD` in `crates/engine/tests/deep.rs`.
 */
export const INTO_A_WON_BOARD = '257 576 765 653 537 376 763 636 367 676 764 640 407 076';

/**
 * Deep: X wins on the top row of middle Boards in 53 plies, with five Free moves inside a middle
 * Board and two reaching the Global board. `LINE_OF_MIDDLE_BOARDS` in
 * `crates/engine/tests/deep.rs`, which pins it.
 */
export const LINE_OF_MIDDLE_BOARDS =
	'280 803 031 313 131 315 151 515 150 504 042 427 271 714 142 426 262 627 272 728 281 815 ' +
	'152 527 270 703 032 327 261 615 130 304 041 413 132 328 282 827 260 604 040 403 030 305 ' +
	'052 522 050 503 051 513 140 405 141';

/** Each move as its path of Spaces. */
export const parse = (moves: string): number[][] =>
	moves
		.split(/\s+/)
		.filter(Boolean)
		.map((digits) => [...digits].map(Number));

/** Each move as its Cell index, the path read in base 9. */
export const cells = (moves: string): number[] =>
	parse(moves).map((path) => path.reduce((index, space) => index * 9 + space, 0));

/** A move's digits, or any path, as Notation: `"42"` is Cell 5·3. */
export const notation = (path: string | number[]) =>
	[...path].map((space) => Number(space) + 1).join('·');
