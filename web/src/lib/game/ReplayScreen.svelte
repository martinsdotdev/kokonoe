<!--
	The Replay viewer (spec §7.4): a saved game a ply at a time, laid out like the game screen. The
	transport sits under the board on every screen, the caption says what the ply shown did, and
	each row of the move list jumps to its ply. The stepped board shows each move where it landed.
	← and → step, and Home and End jump to the start and the end. While autoplay runs, the caption
	stops announcing each ply, so a screen reader doesn't fall behind.
-->
<script lang="ts">
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
	import { playedCount } from './moves.js';
	import MoveList from './MoveList.svelte';
	import PlayerBar from './PlayerBar.svelte';
	import type { Replay } from './replay.svelte.js';
	import { resultText, winner } from './result.js';
	import ResultBanner from './ResultBanner.svelte';
	import { counted } from './rules.js';
	import RulesCard from './RulesCard.svelte';
	import Scoreboard from './Scoreboard.svelte';
	import ScreenHeader from './ScreenHeader.svelte';
	import SteppedBoard from './SteppedBoard.svelte';

	let { replay }: { replay: Replay } = $props();

	/** Narrower than 40rem, Tailwind's `max-sm`: a phone. */
	const phone = new MediaQuery('(width < 40rem)');

	const variant = $derived(replay.variant);
	const against = $derived(againstLabel(replay.opponent));
	const shown = $derived(replay.shown);
	const played = $derived(playedCount(replay.moves));
	/** One Board at a time: on a phone, and at depth 3 everywhere. */
	const stepped = $derived(phone.current || variant.ruleset.depth === 3);
	const atStart = $derived(replay.ply === 0);
	const atEnd = $derived(replay.ply === replay.last);

	const keys: Record<string, () => number> = {
		ArrowLeft: () => replay.ply - 1,
		ArrowRight: () => replay.ply + 1,
		Home: () => 0,
		End: () => replay.last
	};

	/** Where the keys mean something else: typing, or a control of its own. */
	const typing = 'input, textarea, select, [contenteditable], [role="slider"]';

	const onkeydown = (event: KeyboardEvent) => {
		const to = keys[event.key];
		if (!to || event.defaultPrevented) return;
		if (event.altKey || event.ctrlKey || event.metaKey || event.shiftKey) return;
		if (event.target instanceof Element && event.target.closest(typing)) return;
		event.preventDefault();
		replay.go(to());
	};

	// aria-disabled rather than disabled, so a control pressed to the end keeps the focus.
	const control =
		'inline-flex size-11 items-center justify-center rounded-md border-[1.5px] border-foreground aria-disabled:opacity-40';
</script>

<svelte:window {onkeydown} />

<svelte:head>
	<title>Replay: {variant.name} · Kokonoe</title>
</svelte:head>

<div class="min-h-svh">
	<ScreenHeader variant={variant.name} page="Replay" {against} exit="Home" />

	<main class="mx-auto flex max-w-[1320px] flex-col gap-4 p-4 sm:gap-6 sm:p-7">
		<h1 class="sr-only">Replay: {variant.name}, {against}</h1>
		{#if replay.end}
			<ResultBanner
				headline={resultText(replay.end, variant.ruleset, replay.won)}
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
		{@render step('First move', 'Home', atStart, 0, SkipBack)}
		{@render step('Previous move', 'ArrowLeft', atStart, replay.ply - 1, ChevronLeft)}
		<button
			type="button"
			class="{control} bg-primary text-primary-foreground"
			aria-label={replay.playing ? 'Pause' : 'Play'}
			onclick={() => replay.toggle()}
		>
			{#if replay.playing}<Pause aria-hidden="true" />{:else}<Play aria-hidden="true" />{/if}
		</button>
		{@render step('Next move', 'ArrowRight', atEnd, replay.ply + 1, ChevronRight)}
		{@render step('Last move', 'End', atEnd, replay.last, SkipForward)}
	</div>
{/snippet}

{#snippet step(label: string, key: string, off: boolean, to: number, Icon: typeof SkipBack)}
	<button
		type="button"
		class="{control} bg-card"
		aria-label={label}
		aria-keyshortcuts={key}
		aria-disabled={off}
		onclick={() => !off && replay.go(to)}><Icon aria-hidden="true" /></button
	>
{/snippet}

{#snippet caption()}
	<section
		aria-label="This move"
		class="flex flex-col gap-2 rounded-[10px] border-[1.5px] border-foreground bg-card p-4"
	>
		<span class="font-mono text-xs tracking-[0.08em] text-muted-foreground">THIS MOVE</span>
		<p class="m-0 font-semibold" aria-live={replay.playing ? 'off' : 'polite'} data-caption>
			{replay.caption}
		</p>
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
	{#if counted(variant.ruleset)}
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
