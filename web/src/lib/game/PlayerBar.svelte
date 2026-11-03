<!-- One player's bar: their mark, who they are, whether it's their move, and in a timed Room their clock. -->
<script lang="ts">
	import type { Player } from '#lib/engine.js';
	import { clockText, clockWords, LOW_MS } from './clock.js';
	import Mark from './Mark.svelte';
	import type { Seat } from './moves.js';

	interface Props {
		player: Player;
		seat: Seat;
		toMove: boolean;
		/** What the computer is doing on its move, in place of "To move": "Thinking…". */
		doing?: string | null;
		/** A timed Room's time left on this player's clock, in milliseconds. */
		clock?: number | null;
	}

	let { player, seat, toMove, doing = null, clock = null }: Props = $props();

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
			{[seat.detail ?? (seat.yours ? 'You' : null), toMove ? (doing ?? 'To move') : 'Waiting']
				.filter(Boolean)
				.join(' · ')}
		</div>
	</div>
	<!-- The mark is hidden from screen readers, so its letter stays for them beside a clock. -->
	<span class={clock === null ? 'font-mono text-[13px] text-muted-foreground' : 'sr-only'}>
		{player}
	</span>
	{#if clock !== null}
		<span class="sr-only">{clockWords(clock)} left</span>
		<!-- Running low, it's a filled pill: red alone would be O's own ink. -->
		<span
			class="rounded-md px-2 py-0.5 font-mono text-xl font-semibold tabular-nums data-low:bg-o-text data-low:text-card"
			aria-hidden="true"
			data-clock
			data-low={clock < LOW_MS || undefined}
		>
			{clockText(clock)}
		</span>
	{/if}
</div>
