<!-- One player's bar: their mark, who they are, and whether it's their move. -->
<script lang="ts">
	import type { Player } from '#lib/engine.js';
	import Mark from './Mark.svelte';
	import type { Seat } from './moves.js';

	interface Props {
		player: Player;
		seat: Seat;
		toMove: boolean;
		/** What the computer is doing on its move, in place of "To move": "Thinking…". */
		doing?: string | null;
	}

	let { player, seat, toMove, doing = null }: Props = $props();

	const ink = $derived(player === 'X' ? 'var(--x)' : 'var(--o-text)');
</script>

<div
	class="flex items-center gap-3.5 rounded-[10px] bg-card p-4"
	style:border={toMove ? `2px solid ${ink}` : '1.5px solid var(--pencil)'}
	style:box-shadow={toMove ? `4px 4px 0 color-mix(in oklab, ${ink} 18%, transparent)` : 'none'}
	data-to-move={toMove || undefined}
>
	<Mark {player} class="size-8.5 shrink-0" />
	<div class="min-w-0 flex-1">
		<div class="font-semibold">{seat.name}</div>
		<div
			class="text-sm"
			style:color={toMove ? ink : 'var(--muted-foreground)'}
			style:font-weight={toMove ? 600 : 400}
		>
			{[seat.detail, toMove ? (doing ?? 'To move') : 'Waiting'].filter(Boolean).join(' · ')}
		</div>
	</div>
	<span class="font-mono text-[13px] text-muted-foreground">{player}</span>
</div>
