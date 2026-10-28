<!--
	One Board under the Global board in the board grammar: pencil between Cells, the highlighter
	when it's Active, a dashed Sent target, a ring on the last move, an inked owner or a
	Continuation watermark, and hatching once drawn. It renders its part of a BoardView and reports
	clicks.

	With `onstep` the whole tile is one button that steps into it, and its Cells are only drawn:
	the Global view on a phone, where Boards are tapped, not Cells.
-->
<script lang="ts">
	import type { Board, Cell, Player } from '#lib/engine.js';
	import Mark from './Mark.svelte';
	import { boardLabel, boardName, notation } from './notation.js';
	import { at, cellsOf, key, ownerOf, same, type BoardView } from './view.js';

	interface Props {
		view: BoardView;
		board: Board;
		onplay?: (cell: Cell) => void;
		/** Makes the tile a button that steps into its Board, unless it's drawn `large`. */
		onstep?: (board: Board) => void;
		/** A Board to flash, after an illegal tap that names it. A new `tick` flashes again. */
		flash?: { board: Board; tick: number } | null;
		/** The game is over: Cells take no clicks. */
		disabled?: boolean;
		/** Drawn alone, as large as the screen allows. */
		large?: boolean;
	}

	let {
		view,
		board,
		onplay,
		onstep,
		flash = null,
		disabled = false,
		large = false
	}: Props = $props();

	const state = $derived(at(view, board));
	/** The whole tile is a button that steps into it. */
	const button = $derived(!large && !!onstep);
	const won = $derived(ownerOf(state));
	const name = $derived.by(() => {
		const name = boardName(board);
		return won ? `${name}, won by ${won}` : state.status === 'Drawn' ? `${name}, drawn` : name;
	});

	const legal = (cell: Cell) => !disabled && state.active && view.cells[cell] === null;
</script>

<svelte:element
	this={button ? 'button' : 'div'}
	type={button ? 'button' : undefined}
	class="tile"
	class:large
	role={button ? undefined : 'group'}
	aria-label={button && state.active ? `${name}, play here` : name}
	onclick={button ? () => onstep?.(board) : undefined}
	data-board={key(board)}
	data-status={won ? `won-${won.toLowerCase()}` : state.status === 'Drawn' ? 'drawn' : 'open'}
	data-active={state.active || undefined}
	data-sent={same(view.sent, board) || undefined}
	data-flash={same(flash?.board, board) || undefined}
>
	<span class="cells" class:faded={won && !view.continuation}>
		<!-- Keyed by Space, so a follow from one Board to another keeps the same Cell buttons. -->
		{#each cellsOf(board) as cell, space (space)}
			{@const mark = view.cells[cell]}
			{#if button}
				<span class="cell">{@render face(cell, mark)}</span>
			{:else}
				<button
					type="button"
					class="cell"
					aria-label={`Cell ${notation(cell, view.depth)}${mark ? `, ${mark}` : ''}`}
					aria-disabled={!legal(cell)}
					onclick={() => !disabled && onplay?.(cell)}
				>
					{@render face(cell, mark)}
				</button>
			{/if}
		{/each}
	</span>
	{#if state.status === 'Drawn'}
		<span class="hatch" aria-hidden="true"></span>
	{/if}
	{#if won}
		<svg
			viewBox="0 0 100 100"
			class="owner"
			class:watermark={view.continuation}
			aria-hidden="true"
			data-owner={won}
		>
			{#if won === 'X'}
				<path d="M15 17 C 36 37, 60 60, 85 85 M84 15 C 61 39, 40 61, 16 84" stroke="var(--x)" />
			{:else}
				<path
					d="M53 14 C 75 14, 88 31, 87 51 C 86 73, 69 88, 48 87 C 27 86, 13 70, 14 49 C 15 29, 30 15, 49 13"
					stroke="var(--o)"
				/>
			{/if}
		</svg>
	{/if}
	{#if same(view.sent, board)}
		<span class="sent" aria-hidden="true"></span>
		<span class="sent-label">Sent → {boardLabel(board)}</span>
	{/if}
	{#if flash && same(flash.board, board)}
		{#key flash.tick}<span class="flash" aria-hidden="true"></span>{/key}
	{/if}
</svelte:element>

{#snippet face(cell: Cell, mark: Player | null)}
	{#if mark}<Mark player={mark} class="glyph" />{/if}
	{#if cell === view.last}<span class="ring" data-last></span>{/if}
{/snippet}

<style>
	.tile {
		/* The ground under Cells: paper, washed with highlighter where the next move may go. */
		--ground: var(--background);
		--wash: none;
		position: relative;
		display: flex;
		min-width: 0;
		min-height: 0;
		padding: 8%;
		background: var(--wash), var(--ground);
	}
	.tile[data-active] {
		--wash: linear-gradient(var(--highlighter), var(--highlighter));
	}
	button.tile {
		margin: 0;
		border: 0;
		color: inherit;
		font: inherit;
		cursor: pointer;
	}
	button.tile:focus-visible {
		z-index: 1;
		outline: 3px solid var(--ring);
		outline-offset: -3px;
	}
	/* One board alone, framed in the ink that parts Local boards on the Global board. */
	.tile.large {
		width: 100%;
		max-width: 600px;
		aspect-ratio: 1;
		padding: 6%;
		border: 5px solid var(--foreground);
	}

	.cells {
		flex: 1;
		display: grid;
		grid-template: repeat(3, minmax(0, 1fr)) / repeat(3, minmax(0, 1fr));
		gap: 2px;
		background: var(--pencil);
	}
	.cells.faded {
		opacity: 0.3;
	}
	.large .cells {
		gap: 3px;
	}

	.cell {
		position: relative;
		display: flex;
		align-items: center;
		justify-content: center;
		min-width: 0;
		min-height: 0;
		padding: 0;
		border: 0;
		background: var(--wash), var(--ground);
		cursor: default;
	}
	.cell[aria-disabled='false'] {
		cursor: pointer;
	}
	/* Only for a pointer that hovers: after a tap, touch screens leave :hover on the Cell. */
	@media (hover: hover) {
		.cell[aria-disabled='false']:hover {
			--ground: color-mix(in oklab, var(--pencil) 35%, var(--background));
		}
	}
	.cell:focus-visible {
		z-index: 1;
		outline: 3px solid var(--ring);
		outline-offset: -1px;
	}
	.cell :global(.glyph) {
		width: 66%;
		height: 66%;
	}

	.ring {
		position: absolute;
		inset: 7%;
		border: 2px solid var(--foreground);
		border-radius: 50%;
		pointer-events: none;
	}
	.large .ring {
		border-width: 3px;
	}

	.hatch {
		position: absolute;
		inset: 0;
		pointer-events: none;
		background-image: repeating-linear-gradient(
			45deg,
			color-mix(in oklab, var(--foreground) 22%, transparent) 0 2px,
			transparent 2px 15px
		);
	}

	.owner {
		position: absolute;
		top: 9%;
		left: 9%;
		width: 82%;
		height: 82%;
		opacity: 0.95;
		transform: rotate(-3deg);
		pointer-events: none;
	}
	.owner path {
		fill: none;
		stroke-width: 8.5;
		stroke-linecap: round;
	}
	.owner.watermark {
		opacity: 0.2;
	}

	.sent {
		position: absolute;
		inset: 5px;
		border: 3px dashed var(--foreground);
		pointer-events: none;
	}
	.sent-label {
		position: absolute;
		top: 0;
		left: 0;
		padding: 0 6px;
		background: var(--foreground);
		color: var(--background);
		font: 500 12px/1.5 var(--font-mono);
		letter-spacing: 0.06em;
		text-transform: uppercase;
		white-space: nowrap;
		pointer-events: none;
	}
	.large .sent-label {
		font-size: 14px;
	}

	.flash {
		position: absolute;
		inset: 0;
		outline: 3px solid var(--o);
		outline-offset: -3px;
		pointer-events: none;
		animation: flash 700ms ease-out forwards;
	}
	@keyframes flash {
		0%,
		50% {
			opacity: 1;
		}
		25%,
		100% {
			opacity: 0;
		}
	}
	@media (prefers-reduced-motion: reduce) {
		.flash {
			animation: none;
		}
	}
</style>
