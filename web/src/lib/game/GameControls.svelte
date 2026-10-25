<!-- Undo, and Resign behind a confirmation in place, for the player to move. -->
<script lang="ts">
	import type { Player } from '#lib/engine.js';

	interface Props {
		canUndo: boolean;
		/** The player to move, who would resign. Null once the game is over. */
		toMove: Player | null;
		onundo: () => void;
		onresign: () => void;
	}

	let { canUndo, toMove, onundo, onresign }: Props = $props();

	/** Who asked to resign. A move in the meantime changes who's to move and drops the question. */
	let asked: Player | null = $state(null);
	const confirming = $derived(asked !== null && asked === toMove);

	const button =
		'min-h-11 flex-1 rounded-md border-[1.5px] bg-card px-4 font-semibold disabled:opacity-40';
</script>

<div class="flex flex-wrap items-center gap-2.5">
	{#if confirming}
		<span class="flex-[1_1_100%] font-semibold">Resign as {toMove}?</span>
		<button
			type="button"
			class={button}
			style:border-color="var(--o-text)"
			style:color="var(--o-text)"
			onclick={() => {
				asked = null;
				onresign();
			}}>Yes, resign</button
		>
		<button type="button" class="{button} border-foreground" onclick={() => (asked = null)}>
			No
		</button>
	{:else}
		<button type="button" class="{button} border-foreground" disabled={!canUndo} onclick={onundo}>
			Undo
		</button>
		<button
			type="button"
			class={button}
			style:border-color="var(--o-text)"
			style:color="var(--o-text)"
			disabled={!toMove}
			onclick={() => (asked = toMove)}
		>
			Resign
		</button>
	{/if}
</div>
