<!--
	Where the player to move may play, and why: anywhere on the first move, the Sent target, or,
	under the FREE MOVE stamp, any open board when the target is decided.
-->
<script lang="ts">
	import type { Player, Sent } from '#lib/engine.js';
	import { boardLabel } from './notation.js';
	import { opponent } from './result.js';

	interface Props {
		player: Player;
		sent: Sent;
		/** The last move in Notation, which did the sending. */
		last: string | null;
	}

	let { player, sent, last }: Props = $props();

	const turn = $derived.by(() => {
		const by = `${opponent(player)}’s move ${last ?? ''}`.trim();
		if (sent === 'FirstMove') {
			return { free: false, line: 'Play anywhere.', why: 'The first move may go in any Cell.' };
		}
		if ('To' in sent) {
			const board = boardLabel(sent.To);
			return { free: false, line: `Play in board ${board}.`, why: `${by} Sent ${player} there.` };
		}
		const board = boardLabel(sent.Free.target);
		return {
			free: true,
			line: 'Play in any open board.',
			why: `${by} Sent ${player} to board ${board}, which is decided, so every open board is ${player}’s.`
		};
	});
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
	<p class="m-0 font-semibold" aria-live="polite">{turn.line}</p>
	<p class="m-0 text-sm text-muted-foreground">{turn.why}</p>
</section>
