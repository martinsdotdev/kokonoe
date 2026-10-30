/**
 * Playing by keyboard (spec §7.7): a Cell typed as its Notation digits, 5 then 1 then 8, and
 * arrow keys that move focus to the neighbouring Cell or Board on screen.
 */
import type { Attachment } from 'svelte/attachments';
import type { Cell } from '#lib/engine.js';
import { parseNotation } from './notation.js';

/** Where the keys mean something else: typing, or a control of its own. */
const TYPING = 'input, textarea, select, [contenteditable], [role="slider"]';

/**
 * Whether a key is the page's to act on: nothing handled it, no modifier but Shift is held (some
 * keyboards need it for digits), and it wasn't pressed in a field or a control that takes keys.
 */
export const shortcut = (event: KeyboardEvent): boolean =>
	!event.defaultPrevented &&
	!event.altKey &&
	!event.ctrlKey &&
	!event.metaKey &&
	!(event.target instanceof Element && event.target.closest(TYPING));

/**
 * The digits typed so far after `key`, and the Cell once there are `depth` of them. Backspace
 * takes back a digit and Escape clears them. Null for a key that isn't typing a Cell.
 */
export const typed = (
	digits: string,
	key: string,
	depth: 2 | 3
): { digits: string; cell?: Cell } | null => {
	if (/^[1-9]$/.test(key)) {
		const next = digits + key;
		if (next.length < depth) return { digits: next };
		return { digits: '', cell: parseNotation([...next].join('·'), depth) ?? undefined };
	}
	if (key === 'Backspace' && digits) return { digits: digits.slice(0, -1) };
	if (key === 'Escape' && digits) return { digits: '' };
	return null;
};

/** The digits typed so far, with a gap for each still to come: `5·_` or `5·1·_`. */
export const typedSoFar = (digits: string, depth: 2 | 3): string =>
	[...digits, ...Array(depth - digits.length).fill('_')].join('·');

export interface Box {
	x: number;
	y: number;
	width: number;
	height: number;
}

const ARROWS: Record<string, [number, number]> = {
	ArrowLeft: [-1, 0],
	ArrowRight: [1, 0],
	ArrowUp: [0, -1],
	ArrowDown: [0, 1]
};

/**
 * Which of `boxes` an arrow `key` moves to from `from`: the nearest one that way in the same row
 * or column, or -1 at the edge or for another key.
 */
export const nearest = (from: Box, boxes: Box[], key: string): number => {
	const way = ARROWS[key];
	if (!way) return -1;
	const [dx, dy] = way;
	const centre = (box: Box) => [box.x + box.width / 2, box.y + box.height / 2];
	const [fx, fy] = centre(from);
	/** Half the box's height across a row, or its width across a column. */
	const slack = (dx ? from.height : from.width) / 2;
	let best = -1;
	let closest = Infinity;
	for (const [i, box] of boxes.entries()) {
		const [x, y] = centre(box);
		const along = dx ? (x - fx) * dx : (y - fy) * dy;
		const across = dx ? Math.abs(y - fy) : Math.abs(x - fx);
		if (along > 1 && across < slack && along < closest) {
			best = i;
			closest = along;
		}
	}
	return best;
};

/**
 * Moves focus between the buttons in `root` with the arrow keys, as they sit on screen: from a
 * Cell to the next across Boards, or between the Boards of a stepped view. At an edge the key
 * goes on to the page, where the Replay viewer steps with it.
 */
const arrowFocus = (event: KeyboardEvent, root: HTMLElement) => {
	if (!(event.key in ARROWS) || !shortcut(event) || event.shiftKey) return;
	const buttons = [...root.querySelectorAll<HTMLElement>('button')];
	const from = buttons.findIndex((button) => button === event.target);
	if (from < 0) return;
	const boxes = buttons.map((button) => button.getBoundingClientRect());
	const to = buttons[nearest(boxes[from], boxes, event.key)];
	if (!to) return;
	event.preventDefault();
	to.focus();
};

/** The arrow keys on a board: `{@attach arrows}` on the element that holds its buttons. */
export const arrows: Attachment<HTMLElement> = (root) => {
	const onkeydown = (event: KeyboardEvent) => arrowFocus(event, root);
	root.addEventListener('keydown', onkeydown);
	return () => root.removeEventListener('keydown', onkeydown);
};
