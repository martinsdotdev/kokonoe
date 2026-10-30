<!--
	The moves in Notation, dealt ones in parentheses, the one shown highlighted and kept in view: the
	latest during a game, or in the Replay viewer the ply shown, where each row jumps to its ply.
-->
<script lang="ts">
	import { written, type Move } from './moves.js';

	interface Props {
		moves: Move[];
		depth: 2 | 3;
		/** The ply shown, counted from 1; the latest unless given. 0 highlights none. */
		current?: number;
		/** Makes each row a button that shows its ply. */
		onjump?: (ply: number) => void;
	}

	let { moves, depth, current, onjump }: Props = $props();

	const shown = $derived(current ?? moves.length);

	let list: HTMLOListElement | undefined = $state();

	// Scroll the list alone, not the page around it, so autoplay on a phone stays on the board.
	$effect(() => {
		const row = list?.children[shown - 1] as HTMLElement | undefined;
		if (!list || !row) return;
		if (row.offsetTop < list.scrollTop) list.scrollTop = row.offsetTop;
		const below = row.offsetTop + row.offsetHeight - list.clientHeight;
		if (below > list.scrollTop) list.scrollTop = below;
	});
</script>

<section class="overflow-hidden rounded-[10px] border-[1.5px] border-foreground bg-card">
	<div class="flex items-baseline justify-between border-b px-4 py-3">
		<h2 class="m-0 font-display text-xl font-semibold">Moves</h2>
		<span class="font-mono text-xs text-muted-foreground">
			{depth === 3 ? 'board·board·cell' : 'board·cell'}
		</span>
	</div>
	{#if moves.length === 0}
		<p class="m-0 px-4 py-3 text-sm text-muted-foreground">No moves yet.</p>
	{:else}
		<!-- In a game its rows aren't buttons, so the list itself takes focus: a region that
		     scrolls must be reachable by keyboard (WCAG 2.1.1), which Svelte's rule doesn't know. -->
		<!-- svelte-ignore a11y_no_noninteractive_tabindex -->
		<ol
			bind:this={list}
			class="relative m-0 max-h-[360px] list-none overflow-y-auto py-1.5"
			aria-label="Moves"
			tabindex={onjump ? undefined : 0}
		>
			{#each moves as move (move.n)}
				<li style:background={move.n === shown ? 'var(--highlighter)' : undefined}>
					{#if onjump}
						<button
							type="button"
							class="row"
							aria-current={move.n === shown ? 'step' : undefined}
							onclick={() => onjump(move.n)}>{@render row(move, move.n === shown)}</button
						>
					{:else}
						<div class="row">{@render row(move, move.n === shown)}</div>
					{/if}
				</li>
			{/each}
		</ol>
	{/if}
</section>

<!-- On the highlighter, grey text loses its contrast, so the row shown is all in ink. -->
{#snippet row(move: Move, highlighted: boolean)}
	{@const quiet = highlighted ? 'text-foreground' : 'text-muted-foreground'}
	<span class="w-7 font-mono text-[13px] {quiet}">{move.n}</span>
	<span class="w-4 font-semibold" style:color={move.player === 'X' ? 'var(--x)' : 'var(--o-text)'}
		>{move.player}</span
	>
	<span class="font-mono text-[15px]">{written(move)}</span>
	<span class="ml-auto text-[13px] {quiet}">{move.note}</span>
{/snippet}

<style>
	.row {
		display: flex;
		align-items: center;
		gap: 0.75rem;
		width: 100%;
		padding: 0.5rem 1rem;
	}
	button.row {
		margin: 0;
		border: 0;
		background: none;
		color: inherit;
		font: inherit;
		text-align: start;
		cursor: pointer;
	}
	button.row:focus-visible {
		outline: 3px solid var(--ring);
		outline-offset: -3px;
	}
</style>
