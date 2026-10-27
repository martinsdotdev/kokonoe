<!-- Undo, and Resign behind a confirmation in place, for the player to move. -->
<script lang="ts">
	import type { Player } from '#lib/engine.js';

	interface Props {
		canUndo: boolean;
		/**
		 * Who would resign: the player to move, or the player against the computer. Null once the
		 * game is over.
		 */
		resigner: Player | null;
		onundo: () => void;
		onresign: () => void;
	}

	let { canUndo, resigner, onundo, onresign }: Props = $props();

	/** Who asked to resign. In pass and play a move changes who would, and drops the question. */
	let asked: Player | null = $state(null);
	const confirming = $derived(asked !== null && asked === resigner);

	const button =
		'min-h-11 flex-1 rounded-md border-[1.5px] bg-card px-4 font-semibold disabled:opacity-40';
</script>

<div class="flex flex-wrap items-center gap-2.5">
	{#if confirming}
		<span class="flex-[1_1_100%] font-semibold">Resign as {resigner}?</span>
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
			disabled={!resigner}
			onclick={() => (asked = resigner)}
		>
			Resign
		</button>
	{/if}
</div>
