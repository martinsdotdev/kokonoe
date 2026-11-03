<!--
	One player's bar: their mark, who they are, whether it's their move, in a timed Room their clock,
	and in any Room how long they've been away.
-->
<script lang="ts">
	import type { Player } from '#lib/engine.js';
	import { clockText, clockWords, elapsedText, LOW_MS } from './clock.js';
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
		/** How long this player has been away from the Room, in milliseconds. */
		away?: number | null;
	}

	let { player, seat, toMove, doing = null, clock = null, away = null }: Props = $props();

	/** Whose move it is, or that this player waits for the other's, unless they're away. */
	const turn = $derived(toMove ? (doing ?? 'To move') : away === null ? 'Waiting' : null);
	const status = $derived([seat.detail ?? (seat.yours ? 'You' : null), turn].filter(Boolean));

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
			{status.join(' · ')}
			{#if away !== null}
				<!-- Read in words, since a screen reader reads 1:05 as a ratio or a time of day. -->
				{status.length ? ' · ' : ''}<span aria-hidden="true" data-away
					>Away {elapsedText(away)}</span
				>
				<span class="sr-only">Away for {clockWords(away)}</span>
			{/if}
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
