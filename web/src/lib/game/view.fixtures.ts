import type { BoardView } from './view.js';

const range = (n: number) => [...Array(n).keys()];

/** For tests: an empty board, depth 2 unless asked, then `edit` sets up the position under test. */
export const position = (
	edit: (view: BoardView) => void = () => {},
	depth: 2 | 3 = 2
): BoardView => {
	const view: BoardView = {
		depth,
		continuation: false,
		cells: range(9 ** depth).map(() => null),
		boards: range(depth - 1).map((level) =>
			range(9 ** (level + 1)).map(() => ({ status: 'Open', active: false }))
		),
		sent: null,
		within: null,
		last: null
	};
	edit(view);
	return view;
};
