<!--
	Resign behind a confirmation in place, and as the game has them, Undo on this device or a Room's
	draw offer and takeback. A control that can't be used now stays focusable, so focus never drops
	from under the player.
-->
<script lang="ts">
	import type { Player } from '#lib/engine.js';
	import type { Action, Controls } from './screen.js';

	let { controls }: { controls: Controls } = $props();

	const resigner = $derived(controls.resigner);
	/**
	 * Who asked to resign. A change of who would drops the question: a move in pass and play, or a
	 * Room's dropped connection.
	 */
	let asked: Player | null = $derived.by(() => {
		void resigner;
		return null;
	});

	const actions = $derived(
		(
			[
				['Undo', controls.undo],
				['Offer draw', controls.offerDraw],
				['Ask takeback', controls.askTakeback]
			] as [string, Action | undefined][]
		).flatMap(([label, action]) => (action ? [{ label, action }] : []))
	);

	const button =
		'min-h-11 flex-1 rounded-md border-[1.5px] bg-card px-4 font-semibold aria-disabled:opacity-40';
</script>

<div class="flex flex-wrap items-center gap-2.5">
	{#if asked}
		<span class="flex-[1_1_100%] font-semibold">Resign as {asked}?</span>
		<button
			type="button"
			class={button}
			style:border-color="var(--o-text)"
			style:color="var(--o-text)"
			onclick={() => {
				asked = null;
				controls.resign();
			}}>Yes, resign</button
		>
		<button type="button" class="{button} border-foreground" onclick={() => (asked = null)}>
			No
		</button>
	{:else}
		{#each actions as { label, action } (label)}
			<button
				type="button"
				class="{button} border-foreground"
				aria-disabled={!action.enabled}
				onclick={() => action.enabled && action.run()}
			>
				{label}
			</button>
		{/each}
		<button
			type="button"
			class={button}
			style:border-color="var(--o-text)"
			style:color="var(--o-text)"
			aria-disabled={!resigner}
			onclick={() => (asked = resigner)}
		>
			Resign
		</button>
	{/if}
</div>
