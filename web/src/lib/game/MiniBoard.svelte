<!--
	A small, still drawing of a BoardView for previews, such as Home's Continue card: the marks, the
	Active boards' highlighter, Decided boards' owners and drawn boards' hatching. One image to
	assistive technology; `Board` is the board people play on.
-->
<script lang="ts">
	import Mark from './Mark.svelte';
	import { ownerOf, type BoardView } from './view.js';

	let { view, label }: { view: BoardView; label: string } = $props();

	const SPACES = [...Array(9).keys()];
</script>

<div class="mini" role="img" aria-label={label}>
	{#each view.locals as local (local.space)}
		{@const won = ownerOf(local)}
		<div class="local" data-active={local.active || undefined}>
			<div class="cells" class:faded={won && !view.continuation}>
				{#each SPACES as inner (inner)}
					{@const mark = view.cells[local.space * 9 + inner]}
					<span class="cell"
						>{#if mark}<Mark player={mark} class="glyph" />{/if}</span
					>
				{/each}
			</div>
			{#if local.status === 'Drawn'}<span class="hatch"></span>{/if}
			{#if won}<Mark player={won} class="owner" />{/if}
		</div>
	{/each}
</div>

<style>
	.mini {
		display: grid;
		grid-template: repeat(3, minmax(0, 1fr)) / repeat(3, minmax(0, 1fr));
		gap: 2px;
		width: 100%;
		aspect-ratio: 1;
		background: var(--foreground);
	}

	.local {
		--ground: var(--background);
		--wash: none;
		position: relative;
		display: flex;
		min-width: 0;
		padding: 6%;
		background: var(--wash), var(--ground);
	}
	.local[data-active] {
		--wash: linear-gradient(var(--highlighter), var(--highlighter));
	}

	.cells {
		flex: 1;
		display: grid;
		grid-template: repeat(3, minmax(0, 1fr)) / repeat(3, minmax(0, 1fr));
		gap: 1px;
		background: var(--pencil);
	}
	.cells.faded {
		opacity: 0.3;
	}

	.cell {
		display: flex;
		align-items: center;
		justify-content: center;
		min-width: 0;
		background: var(--wash), var(--ground);
	}
	.cell :global(.glyph) {
		width: 80%;
		height: 80%;
	}

	.hatch {
		position: absolute;
		inset: 0;
		background-image: repeating-linear-gradient(
			45deg,
			color-mix(in oklab, var(--foreground) 22%, transparent) 0 1px,
			transparent 1px 6px
		);
	}

	.local :global(.owner) {
		position: absolute;
		inset: 8%;
		width: 84%;
		height: 84%;
	}
</style>
