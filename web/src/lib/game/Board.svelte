<!--
	The Global board at depth 2, drawn in the design overview's grammar: heavy ink between Local
	boards, and each Local board in its own. It renders a BoardView and reports clicks: what a click
	means is the caller's business.
-->
<script lang="ts">
	import type { Cell } from '#lib/engine.js';
	import LocalBoard from './LocalBoard.svelte';
	import type { BoardView } from './view.js';

	interface Props {
		view: BoardView;
		onplay?: (cell: Cell) => void;
		/** A Local board to flash, after an illegal tap that names it. A new `tick` flashes again. */
		flash?: { space: number; tick: number } | null;
		/** The game is over: Cells take no clicks. */
		disabled?: boolean;
	}

	let { view, onplay, flash = null, disabled = false }: Props = $props();
</script>

<div class="board" role="group" aria-label="The Global board">
	{#each view.locals as local (local.space)}
		<LocalBoard {view} {local} {onplay} {flash} {disabled} />
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
