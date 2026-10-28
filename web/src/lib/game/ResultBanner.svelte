<!--
	The result in words once the game ends, and, when given, the ways on: its Replay, a new game,
	and the game as a file.
-->
<script lang="ts">
	import { resolve } from '$app/paths';
	import type { Player } from '#lib/engine.js';
	import ExportButtons from './ExportButtons.svelte';
	import type { GameFile } from './gamefile.js';
	import { replayQuery } from './launch.js';

	interface Props {
		headline: string;
		detail: string;
		winner: Player | null;
		/** The saved game's id, for a link to its Replay. */
		game?: string;
		/** Starts a new game; without it the banner has no button. */
		onrestart?: () => void;
		/** The game as a file, to export. */
		exported?: GameFile;
	}

	let { headline, detail, winner, game, onrestart, exported }: Props = $props();

	const ink = $derived(
		winner === 'X' ? 'var(--x)' : winner === 'O' ? 'var(--o-text)' : 'var(--foreground)'
	);
</script>

<section
	aria-label="Result"
	class="flex flex-wrap items-center justify-between gap-5 rounded-xl border-2 border-foreground bg-card px-6 py-5"
	style:box-shadow="6px 6px 0 var(--muted)"
>
	<div class="flex min-w-0 items-center gap-5">
		<span
			class="flex size-16 flex-none items-center justify-center rounded-full border-2 font-display text-3xl font-bold"
			style:border-color={ink}
			style:color={ink}
			aria-hidden="true">{winner ?? '='}</span
		>
		<div class="min-w-0">
			<h2
				class="m-0 font-display text-[40px] leading-[1.05] font-semibold tracking-[-0.015em]"
				aria-live="polite"
			>
				{headline}
			</h2>
			<p class="mt-1 mb-0 text-muted-foreground">{detail}</p>
		</div>
	</div>
	{#if game || onrestart}
		<div class="flex flex-wrap gap-3">
			{#if game}
				<a
					href={resolve(`/replay${replayQuery(game)}`)}
					class="inline-flex min-h-12 items-center rounded-md border-[1.5px] border-foreground bg-card px-5 font-semibold text-foreground no-underline"
				>
					Replay
				</a>
			{/if}
			{#if onrestart}
				<button
					type="button"
					class="min-h-12 rounded-md bg-primary px-5 font-semibold text-primary-foreground"
					onclick={onrestart}
				>
					New game
				</button>
			{/if}
		</div>
	{/if}
	{#if exported}
		<ExportButtons file={exported} />
	{/if}
</section>
