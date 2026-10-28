import type { BoardView } from './view.js';

const range = (n: number) => [...Array(n).keys()];

/** For tests: an empty depth-2 board, then `edit` sets up the position under test. */
export const position = (edit: (view: BoardView) => void = () => {}): BoardView => {
	const view: BoardView = {
		continuation: false,
		cells: range(81).map(() => null),
		locals: range(9).map((space) => ({ space, status: 'Open', active: false })),
		sent: null,
		free: false,
		last: null
	};
	edit(view);
	return view;
};
