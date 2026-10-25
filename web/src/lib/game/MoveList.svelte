<!-- The moves so far in Notation, dealt ones in parentheses, the latest highlighted and in view. -->
<script lang="ts">
	import type { Move } from './session.svelte.js';

	let { moves }: { moves: Move[] } = $props();

	let list: HTMLOListElement | undefined = $state();

	$effect(() => {
		if (list && moves.length) list.scrollTop = list.scrollHeight;
	});
</script>

<section class="overflow-hidden rounded-[10px] border-[1.5px] border-foreground bg-card">
	<div class="flex items-baseline justify-between border-b px-4 py-3">
		<h2 class="m-0 font-display text-xl font-semibold">Moves</h2>
		<span class="font-mono text-xs text-muted-foreground">board·cell</span>
	</div>
	{#if moves.length === 0}
		<p class="m-0 px-4 py-3 text-sm text-muted-foreground">No moves yet.</p>
	{:else}
		<ol bind:this={list} class="m-0 max-h-[360px] list-none overflow-y-auto py-1.5">
			{#each moves as move, i (move.n)}
				<li
					class="flex items-center gap-3 px-4 py-2"
					style:background={i === moves.length - 1 ? 'var(--highlighter)' : undefined}
				>
					<span class="w-7 font-mono text-[13px] text-muted-foreground">{move.n}</span>
					<span
						class="w-4 font-semibold"
						style:color={move.player === 'X' ? 'var(--x)' : 'var(--o-text)'}>{move.player}</span
					>
					<span class="font-mono text-[15px]">
						{move.dealt ? `(${move.notation})` : move.notation}
					</span>
					<span class="ml-auto text-[13px] text-muted-foreground">{move.note}</span>
				</li>
			{/each}
		</ol>
	{/if}
</section>
