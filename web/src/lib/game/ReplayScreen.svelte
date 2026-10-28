<!--
	The Replay viewer (spec §7.4): a saved game a ply at a time, laid out like the game screen. The
	transport sits under the board on every screen, the caption says what the ply shown did, and
	each row of the move list jumps to its ply. The stepped board shows each move where it landed.
	← and → step, and Home and End jump to the start and the end.
-->
<script lang="ts">
	import { resolve } from '$app/paths';
	import { MediaQuery } from 'svelte/reactivity';
	import ChevronLeft from '@lucide/svelte/icons/chevron-left';
	import ChevronRight from '@lucide/svelte/icons/chevron-right';
	import Pause from '@lucide/svelte/icons/pause';
	import Play from '@lucide/svelte/icons/play';
	import SkipBack from '@lucide/svelte/icons/skip-back';
	import SkipForward from '@lucide/svelte/icons/skip-forward';
	import Board from './Board.svelte';
	import { againstLabel } from './launch.js';
	import { moves } from './listing.js';
	import MoveList from './MoveList.svelte';
	import PlayerBar from './PlayerBar.svelte';
	import type { Replay } from './replay.svelte.js';
	import { resultText, winner } from './result.js';
	import ResultBanner from './ResultBanner.svelte';
	import RulesCard from './RulesCard.svelte';
	import Scoreboard from './Scoreboard.svelte';
	import SteppedBoard from './SteppedBoard.svelte';

	let { replay }: { replay: Replay } = $props();

	/** Narrower than 40rem, Tailwind's `max-sm`: a phone. */
	const phone = new MediaQuery('(width < 40rem)');

	const variant = $derived(replay.variant);
	const against = $derived(againstLabel(replay.opponent));
	const shown = $derived(replay.shown);
	const played = $derived(replay.moves.filter((move) => !move.dealt).length);
	/** One Board at a time: on a phone, and at depth 3 everywhere. */
	const stepped = $derived(phone.current || variant.ruleset.depth === 3);
	const counted = $derived(
		variant.ruleset.objective !== 'Line' || variant.ruleset.fallback === 'MostWon'
	);
	const atStart = $derived(replay.ply === 0);
	const atEnd = $derived(replay.ply === replay.last);

	const keys: Record<string, () => number> = {
		ArrowLeft: () => replay.ply - 1,
		ArrowRight: () => replay.ply + 1,
		Home: () => 0,
		End: () => replay.last
	};

	const onkeydown = (event: KeyboardEvent) => {
		const to = keys[event.key];
		if (!to || event.altKey || event.ctrlKey || event.metaKey || event.shiftKey) return;
		event.preventDefault();
		replay.go(to());
	};

	const control =
		'inline-flex size-11 items-center justify-center rounded-md border-[1.5px] border-foreground disabled:opacity-40';
</script>

<svelte:window {onkeydown} />

<div class="min-h-svh">
	<header class="border-b-[1.5px] border-foreground bg-card">
		<div
			class="mx-auto flex max-w-[1320px] flex-wrap items-center justify-between gap-x-4 gap-y-1 px-4 py-2 sm:gap-y-4 sm:px-7 sm:py-3"
		>
			<div class="flex flex-wrap items-center gap-4">
				<a
					href={resolve('/')}
					class="font-display text-[26px] font-bold text-foreground no-underline">Kokonoe</a
				>
				<span
					class="rounded border-[1.5px] border-foreground px-2.5 py-0.5 font-mono text-[13px]"
					data-variant
				>
					{variant.name}
				</span>
				<span class="text-muted-foreground">
					Replay<span class="max-sm:hidden">{` · ${against}`}</span>
				</span>
			</div>
			<a
				href={resolve('/')}
				class="inline-flex min-h-11 items-center px-3.5 font-medium text-foreground"
			>
				Home
			</a>
		</div>
	</header>

	<main class="mx-auto flex max-w-[1320px] flex-col gap-4 p-4 sm:gap-6 sm:p-7">
		<h1 class="sr-only">Replay: {variant.name}, {against}</h1>
		{#if replay.end}
			<ResultBanner
				headline={resultText(replay.end, variant.ruleset, replay.moments[replay.last].won)}
				detail={`After ${moves(played)}.`}
				winner={winner(replay.end)}
			/>
		{/if}
		{#if phone.current}
			<section aria-label="Board" class="flex flex-col items-center gap-3">
				{@render board()}
				{@render transport()}
			</section>
			{@render caption()}
			<aside aria-label="Players" class="flex flex-col gap-3.5">{@render players()}</aside>
			<aside aria-label="Moves and rules" class="flex flex-col gap-3.5">
				{@render list()}
				<RulesCard {variant} />
			</aside>
		{:else}
			<div class="flex flex-wrap items-start justify-center gap-8">
				<aside
					aria-label="Players and this move"
					class="flex min-w-0 flex-[1_1_260px] flex-col gap-3.5 lg:max-w-80"
				>
					{@render players()}
					{@render caption()}
				</aside>
				<section
					aria-label="Board"
					class="flex min-w-0 flex-[0_1_600px] flex-col items-center gap-3"
				>
					{@render board()}
					{@render transport()}
				</section>
				<aside
					aria-label="Moves and rules"
					class="flex min-w-0 flex-[1_1_260px] flex-col gap-3.5 lg:max-w-80"
				>
					{@render list()}
					<RulesCard {variant} />
				</aside>
			</div>
		{/if}
	</main>
</div>

{#snippet board()}
	<div class="board">
		{#if stepped}
			<SteppedBoard view={shown.view} disabled landed />
		{:else}
			<Board view={shown.view} disabled />
		{/if}
	</div>
{/snippet}

{#snippet transport()}
	<div class="flex gap-2" role="group" aria-label="Replay controls">
		<button
			type="button"
			class="{control} bg-card"
			aria-label="First move"
			disabled={atStart}
			onclick={() => replay.go(0)}><SkipBack aria-hidden="true" /></button
		>
		<button
			type="button"
			class="{control} bg-card"
			aria-label="Previous move"
			disabled={atStart}
			onclick={() => replay.go(replay.ply - 1)}><ChevronLeft aria-hidden="true" /></button
		>
		<button
			type="button"
			class="{control} bg-primary text-primary-foreground"
			aria-label={replay.playing ? 'Pause' : 'Play'}
			onclick={() => replay.toggle()}
		>
			{#if replay.playing}<Pause aria-hidden="true" />{:else}<Play aria-hidden="true" />{/if}
		</button>
		<button
			type="button"
			class="{control} bg-card"
			aria-label="Next move"
			disabled={atEnd}
			onclick={() => replay.go(replay.ply + 1)}><ChevronRight aria-hidden="true" /></button
		>
		<button
			type="button"
			class="{control} bg-card"
			aria-label="Last move"
			disabled={atEnd}
			onclick={() => replay.go(replay.last)}><SkipForward aria-hidden="true" /></button
		>
	</div>
{/snippet}

{#snippet caption()}
	<section
		aria-label="This move"
		class="flex flex-col gap-2 rounded-[10px] border-[1.5px] border-foreground bg-card p-4"
	>
		<span class="font-mono text-xs tracking-[0.08em] text-muted-foreground">THIS MOVE</span>
		<p class="m-0 font-semibold" aria-live="polite" data-caption>{replay.caption}</p>
	</section>
{/snippet}

{#snippet players()}
	{#each ['X', 'O'] as const as player (player)}
		<PlayerBar
			{player}
			seat={replay.seats[player]}
			toMove={'ToMove' in shown.status && shown.status.ToMove.player === player}
		/>
	{/each}
	{#if counted}
		<Scoreboard won={shown.won} ruleset={variant.ruleset} />
	{/if}
{/snippet}

{#snippet list()}
	<MoveList
		moves={replay.moves}
		depth={variant.ruleset.depth}
		current={replay.ply}
		onjump={(ply) => replay.go(ply)}
	/>
{/snippet}

<style>
	/* The board and the transport under it fit the window together, as on the game screen the
	   board alone does, but never shrink past a readable size: a short window scrolls instead. */
	.board {
		display: flex;
		justify-content: center;
		width: 100%;
		max-width: min(600px, max(20rem, 100svh - 14rem));
	}
</style>
