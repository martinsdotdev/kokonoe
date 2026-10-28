<!--
	One Local board in the board grammar: pencil between Cells, the highlighter when it's Active, a
	dashed Sent target, a ring on the last move, an inked owner or a Continuation watermark, and
	hatching once drawn. It renders its part of a BoardView and reports clicks.

	With `onstep` the whole board is one button that steps into it, and its Cells are only drawn:
	the Global view on a phone, where Boards are tapped, not Cells.
-->
<script lang="ts">
	import type { Cell, Player } from '#lib/engine.js';
	import Mark from './Mark.svelte';
	import { boardLabel, notation } from './notation.js';
	import { ownerOf, type BoardView, type LocalView } from './view.js';

	interface Props {
		view: BoardView;
		local: LocalView;
		onplay?: (cell: Cell) => void;
		/** Makes the board a button that steps into it, by its Space. */
		onstep?: (space: number) => void;
		/** A Local board to flash, after an illegal tap that names it. A new `tick` flashes again. */
		flash?: { space: number; tick: number } | null;
		/** The game is over: Cells take no clicks. */
		disabled?: boolean;
		/** Drawn alone, as large as the screen allows. */
		large?: boolean;
	}

	let {
		view,
		local,
		onplay,
		onstep,
		flash = null,
		disabled = false,
		large = false
	}: Props = $props();

	const SPACES = [...Array(9).keys()];

	const won = $derived(ownerOf(local));
	const name = $derived.by(() => {
		const name = `Board ${boardLabel([local.space])}`;
		return won ? `${name}, won by ${won}` : local.status === 'Drawn' ? `${name}, drawn` : name;
	});

	const legal = (cell: Cell) => !disabled && local.active && view.cells[cell] === null;
</script>

<svelte:element
	this={onstep ? 'button' : 'div'}
	type={onstep ? 'button' : undefined}
	class="local"
	class:large
	role={onstep ? undefined : 'group'}
	aria-label={onstep && local.active ? `${name}, play here` : name}
	onclick={onstep && (() => onstep(local.space))}
	data-space={local.space}
	data-status={won ? `won-${won.toLowerCase()}` : local.status === 'Drawn' ? 'drawn' : 'open'}
	data-active={local.active || undefined}
	data-sent={view.sent === local.space || undefined}
	data-flash={flash?.space === local.space || undefined}
>
	<span class="cells" class:faded={won && !view.continuation}>
		{#each SPACES as inner (inner)}
			{@const cell = local.space * 9 + inner}
			{@const mark = view.cells[cell]}
			{#if onstep}
				<span class="cell">{@render face(cell, mark)}</span>
			{:else}
				<button
					type="button"
					class="cell"
					aria-label={`Cell ${notation(cell, 2)}${mark ? `, ${mark}` : ''}`}
					aria-disabled={!legal(cell)}
					onclick={() => !disabled && onplay?.(cell)}
				>
					{@render face(cell, mark)}
				</button>
			{/if}
		{/each}
	</span>
	{#if local.status === 'Drawn'}
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
	{#if view.sent === local.space}
		<span class="sent" aria-hidden="true"></span>
		<span class="sent-label">Sent → {boardLabel([local.space])}</span>
	{/if}
	{#if flash?.space === local.space}
		{#key flash.tick}<span class="flash" aria-hidden="true"></span>{/key}
	{/if}
</svelte:element>

{#snippet face(cell: Cell, mark: Player | null)}
	{#if mark}<Mark player={mark} class="glyph" />{/if}
	{#if cell === view.last}<span class="ring" data-last></span>{/if}
{/snippet}

<style>
	.local {
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
	.local[data-active] {
		--wash: linear-gradient(var(--highlighter), var(--highlighter));
	}
	button.local {
		margin: 0;
		border: 0;
		color: inherit;
		font: inherit;
		cursor: pointer;
	}
	button.local:focus-visible {
		z-index: 1;
		outline: 3px solid var(--ring);
		outline-offset: -3px;
	}
	/* One board alone, framed in the ink that parts Local boards on the Global board. */
	.local.large {
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
