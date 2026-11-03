<!--
	The result in words once the game ends, and, when given, the ways on: its Replay, a new game or
	a Room's rematch, and the game as a file.
-->
<script lang="ts">
	import { resolve } from '$app/paths';
	import type { GameResult } from '#lib/engine.js';
	import type { GameFile } from './gamefile.js';
	import { replayQuery } from './launch.js';
	import OfferCard from './OfferCard.svelte';
	import type { PendingOffer } from './offers.js';
	import type { Action } from './screen.js';

	interface Props {
		headline: string;
		detail: string;
		/** The game file's Result, which the disc shows: the winner, a draw, or no result. */
		result: GameResult | '*';
		/** The saved game's id, for a link to its Replay. */
		game?: string;
		/** Starts a new game; without it the banner has no button. */
		onrestart?: () => void;
		/** The game as a file, to export. */
		file?: GameFile;
		/** A Room's rematch: offering it, the other player's offer, and the Room it made. */
		rematch?: {
			offer: Action;
			waiting: PendingOffer | null;
			onanswer: (accept: boolean) => void;
			room: string | null;
		};
	}

	let { headline, detail, result, game, onrestart, file, rematch }: Props = $props();

	/** The disc's mark for each Result: an abandoned game's has none, so it isn't a draw's. */
	const DISCS = { '1-0': 'X', '0-1': 'O', '1/2-1/2': '=', '*': '–' } as const;
	const disc = $derived(DISCS[result]);
	const ink = $derived(
		disc === 'X' ? 'var(--x)' : disc === 'O' ? 'var(--o-text)' : 'var(--foreground)'
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
			aria-hidden="true"
			data-disc>{disc}</span
		>
		<div class="min-w-0">
			<!-- Not a live region: one that appears already filled may go unread. The game screen's
			     live line says the result. -->
			<h2 class="m-0 font-display text-[40px] leading-[1.05] font-semibold tracking-[-0.015em]">
				{headline}
			</h2>
			<p class="mt-1 mb-0 text-muted-foreground">{detail}</p>
		</div>
	</div>
	{#if game || onrestart || rematch}
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
			{#if rematch?.room}
				<a
					href={resolve('/r/[room]', { room: rematch.room })}
					class="inline-flex min-h-12 items-center rounded-md bg-primary px-5 font-semibold text-primary-foreground no-underline"
				>
					Play the rematch
				</a>
			{:else if rematch}
				{@const { offer } = rematch}
				<button
					type="button"
					class="min-h-12 rounded-md bg-primary px-5 font-semibold text-primary-foreground aria-disabled:opacity-40"
					aria-disabled={!offer.enabled}
					onclick={() => offer.enabled && offer.run()}
				>
					Rematch
				</button>
			{/if}
		</div>
	{/if}
	{#if rematch?.waiting}
		<div class="basis-full">
			<OfferCard offer={rematch.waiting} onanswer={rematch.onanswer} />
		</div>
	{/if}
	{#if file}
		<!-- Loaded once a game ends, so the game screen doesn't load it to start. -->
		{#await import('./ExportButtons.svelte') then { default: ExportButtons }}
			<ExportButtons {file} />
		{:catch}
			<p class="m-0 text-sm text-muted-foreground">Export couldn’t load.</p>
		{/await}
	{/if}
</section>
