<!--
	The Global board, drawn in the design overview's grammar: heavy ink between its nine Boards,
	Local boards at depth 2 and middle Boards at depth 3, each drawn by a BoardTile. It renders a
	BoardView and reports clicks: what a click means is the caller's business. With `onstep`,
	Boards are tapped instead of Cells. The arrow keys move between its buttons as they sit.
-->
<script lang="ts">
	import type { Board, Cell } from '#lib/engine.js';
	import BoardTile from './BoardTile.svelte';
	import { arrows } from './keys.js';
	import { children, key, type BoardView } from './view.js';

	interface Props {
		view: BoardView;
		onplay?: (cell: Cell) => void;
		/** Makes each Board on it a button that steps into that Board. */
		onstep?: (board: Board) => void;
		/** A Board to flash, after an illegal tap that names it. A new `tick` flashes again. */
		flash?: { board: Board; tick: number } | null;
		/** The game is over: Cells take no clicks. */
		disabled?: boolean;
	}

	let { view, onplay, onstep, flash = null, disabled = false }: Props = $props();
</script>

<div class="board" role="group" aria-label="The Global board" {@attach arrows}>
	{#each children([]) as board (key(board))}
		<BoardTile {view} {board} {onplay} {onstep} {flash} {disabled} />
	{/each}
</div>

<style>
	.board {
		display: grid;
		grid-template: repeat(3, minmax(0, 1fr)) / repeat(3, minmax(0, 1fr));
		gap: 5px;
		width: 100%;
		max-width: 600px;
		aspect-ratio: 1;
		background: var(--foreground);
	}
</style>
