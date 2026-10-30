<!--
	Where the player to move may play, and why: anywhere on the first move, the Sent target, or,
	under the FREE MOVE stamp, anywhere open in the Board above a decided target. It also shows a
	Cell being typed. The game screen's live line, not this card, says it to a screen reader.
-->
<script lang="ts">
	import type { Player } from '#lib/engine.js';
	import type { Turn } from './announce.js';

	interface Props {
		player: Player;
		/** Where to play and why, from `turn`. */
		turn: Turn;
		/** A Cell being typed, with a gap for each digit to come: `5·_`. */
		typed?: string | null;
	}

	let { player, turn, typed = null }: Props = $props();
</script>

<section
	aria-label="Turn"
	class="flex flex-col gap-2 rounded-[10px] border-[1.5px] border-foreground bg-card p-4"
>
	<span class="font-mono text-xs tracking-[0.08em] text-muted-foreground">THIS TURN · {player}</span
	>
	{#if turn.free}
		<span
			class="-rotate-5 self-start border-2 px-3 py-1 font-mono text-sm font-medium tracking-[0.14em]"
			style:border-color="var(--o-text)"
			style:color="var(--o-text)"
			data-stamp
		>
			FREE MOVE
		</span>
	{/if}
	<p class="m-0 font-semibold">{turn.line}</p>
	<p class="m-0 text-sm text-muted-foreground">{turn.why}</p>
	{#if typed}
		<p class="m-0 font-mono text-sm" data-typed>Typing {typed}</p>
	{/if}
</section>
