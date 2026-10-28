<!--
	A small, still drawing of a Board for previews, such as Home's Continue card: the Active boards'
	highlighter, Decided boards' owners and drawn boards' hatching, and the marks in its Local
	boards, dealt ones in pencil grey. A whole Deep board draws its Local boards without marks, which
	would be too small to see. One image to assistive technology, or with no `label` none; `Board`
	is the board people play on.
-->
<script lang="ts">
	import type { Board } from '#lib/engine.js';
	import Mark from './Mark.svelte';
	import { at, cellsOf, children, key, ownerOf, type BoardView } from './view.js';

	interface Props {
		view: BoardView;
		/** The Board to draw: the Global board unless named. */
		board?: Board;
		/** What the image shows; without one it's only decoration. */
		label?: string;
	}

	let { view, board = [], label }: Props = $props();

	/** Marks are drawn one level down from the Board drawn, and no further. */
	const marked = (tile: Board) => tile.length === board.length + 1;
</script>

<div
	class="mini"
	role={label ? 'img' : undefined}
	aria-label={label}
	aria-hidden={label ? undefined : 'true'}
>
	{#each children(board) as child (key(child))}
		{@render tile(child)}
	{/each}
</div>

{#snippet tile(here: Board)}
	{@const state = at(view, here)}
	{@const won = ownerOf(state)}
	<div class="tile" data-active={state.active || undefined}>
		{#if here.length < view.depth - 1}
			<div class="boards grid" class:faded={won && !view.continuation}>
				{#each children(here) as child (key(child))}
					{@render tile(child)}
				{/each}
			</div>
		{:else if marked(here)}
			<div class="grid" class:faded={won && !view.continuation}>
				{#each cellsOf(here) as cell (cell)}
					{@const mark = view.cells[cell]}
					<span class="cell"
						>{#if mark}<Mark
								player={mark}
								dealt={view.dealt.includes(cell)}
								class="glyph"
							/>{/if}</span
					>
				{/each}
			</div>
		{/if}
		{#if state.status === 'Drawn'}<span class="hatch"></span>{/if}
		{#if won}<Mark player={won} class="owner" />{/if}
	</div>
{/snippet}

<style>
	.mini {
		display: grid;
		grid-template: repeat(3, minmax(0, 1fr)) / repeat(3, minmax(0, 1fr));
		gap: 2px;
		width: 100%;
		aspect-ratio: 1;
		background: var(--foreground);
	}

	.tile {
		--ground: var(--background);
		--wash: none;
		position: relative;
		display: flex;
		min-width: 0;
		min-height: 0;
		padding: 6%;
		background: var(--wash), var(--ground);
	}
	.tile[data-active] {
		--wash: linear-gradient(var(--highlighter), var(--highlighter));
	}

	.grid {
		flex: 1;
		display: grid;
		grid-template: repeat(3, minmax(0, 1fr)) / repeat(3, minmax(0, 1fr));
		gap: 1px;
		background: var(--pencil);
	}
	/* Local boards inside a middle Board, parted in ink rather than pencil. */
	.grid.boards {
		background: var(--foreground);
	}
	.grid.boards .tile {
		padding: 0;
	}
	.grid.faded {
		opacity: 0.3;
	}
	/* A faded middle Board fades its Local boards once, not twice. */
	.grid.faded .grid.faded {
		opacity: 1;
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

	.tile :global(.owner) {
		position: absolute;
		inset: 8%;
		width: 84%;
		height: 84%;
	}
</style>
