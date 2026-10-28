<!--
	The game screen (spec §7.4). Wider than a phone it has the design overview's three columns,
	which wrap on narrower screens: players and the turn, the board, then the moves and the
	controls. On a phone the board comes first and steps into one Board at a time (§7.3), with the
	turn under it, then the players, the controls, the moves and the rules, in that order in the
	page as on screen. Deep steps on every screen: its Cells are too small to play on the whole
	board.
-->
<script lang="ts">
	import { resolve } from '$app/paths';
	import { MediaQuery } from 'svelte/reactivity';
	import type { Player } from '#lib/engine.js';
	import Board from './Board.svelte';
	import GameControls from './GameControls.svelte';
	import MoveList from './MoveList.svelte';
	import PlayerBar from './PlayerBar.svelte';
	import ResultBanner from './ResultBanner.svelte';
	import RulesCard from './RulesCard.svelte';
	import Scoreboard from './Scoreboard.svelte';
	import SteppedBoard from './SteppedBoard.svelte';
	import { againstLabel } from './launch.js';
	import { moves } from './listing.js';
	import { resultText, winner } from './result.js';
	import type { OfflineGame } from './session.svelte.js';
	import TurnCard from './TurnCard.svelte';
	import { decidedAbove } from './view.js';

	let { session }: { session: OfflineGame } = $props();

	/** Narrower than 40rem, Tailwind's `max-sm`: a phone. */
	const phone = new MediaQuery('(width < 40rem)');

	const played = $derived(session.moves.filter((move) => !move.dealt).length);
	const opponent = $derived(session.opponent);
	const variant = $derived(session.variant);
	const against = $derived(againstLabel(opponent));
	/** The board takes no moves: the game is over, or the computer is to move. */
	const frozen = $derived(session.end !== null || session.computerToMove);
	/** One Board at a time: on a phone, and at depth 3 everywhere. */
	const stepped = $derived(phone.current || variant.ruleset.depth === 3);
	const counted = $derived(
		variant.ruleset.objective !== 'Line' || variant.ruleset.fallback === 'MostWon'
	);

	/** What the computer is doing on its move, for its player bar. */
	const doing = (player: Player) =>
		session.computerToMove && session.toMove === player
			? session.stuck
				? 'Couldn’t move'
				: 'Thinking…'
			: null;
</script>

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
				<span class="text-muted-foreground max-sm:hidden">{against}</span>
			</div>
			<a
				href={resolve('/')}
				class="inline-flex min-h-11 items-center px-3.5 font-medium text-foreground"
			>
				Leave game
			</a>
		</div>
	</header>

	<main class="mx-auto flex max-w-[1320px] flex-col gap-4 p-4 sm:gap-6 sm:p-7">
		<h1 class="sr-only">{variant.name}, {against}</h1>
		{#if session.end}
			<ResultBanner
				headline={resultText(session.end, variant.ruleset, session.won)}
				detail={`After ${moves(played)}.`}
				winner={winner(session.end)}
				onrestart={() => session.restart()}
			/>
		{/if}
		{#if phone.current}
			<section aria-label="Board" class="flex justify-center">{@render board()}</section>
			{@render turn()}
			<aside aria-label="Players" class="flex flex-col gap-3.5">{@render players()}</aside>
			<aside aria-label="Moves and controls" class="flex flex-col gap-3.5">
				{@render controls()}
				<MoveList moves={session.moves} depth={variant.ruleset.depth} />
				<RulesCard {variant} />
			</aside>
		{:else}
			<div class="flex flex-wrap items-start justify-center gap-8">
				<aside
					aria-label="Players and turn"
					class="flex min-w-0 flex-[1_1_260px] flex-col gap-3.5 lg:max-w-80"
				>
					{@render players()}
					{@render turn()}
				</aside>
				<section aria-label="Board" class="flex min-w-0 flex-[0_1_600px] justify-center">
					{@render board()}
				</section>
				<aside
					aria-label="Moves and controls"
					class="flex min-w-0 flex-[1_1_260px] flex-col gap-3.5 lg:max-w-80"
				>
					<MoveList moves={session.moves} depth={variant.ruleset.depth} />
					{@render controls()}
					<RulesCard {variant} />
				</aside>
			</div>
		{/if}
	</main>
</div>

{#snippet board()}
	{#if stepped}
		<SteppedBoard
			view={session.view}
			flash={session.flash}
			refusal={session.refusal}
			disabled={frozen}
			replied={session.replied}
			onplay={(cell) => session.play(cell)}
		/>
	{:else}
		<Board
			view={session.view}
			flash={session.flash}
			disabled={frozen}
			onplay={(cell) => session.play(cell)}
		/>
	{/if}
{/snippet}

{#snippet players()}
	{#each ['X', 'O'] as const as player (player)}
		<PlayerBar
			{player}
			seat={session.seats[player]}
			toMove={session.toMove === player}
			doing={doing(player)}
		/>
	{/each}
	{#if session.stuck && !session.end}
		<p class="m-0 flex flex-wrap items-center gap-3 text-sm" role="alert">
			The computer couldn’t find its move.
			<button
				type="button"
				class="min-h-11 rounded-md border-[1.5px] border-foreground bg-card px-4 font-semibold"
				onclick={() => session.retry()}
			>
				Try again
			</button>
		</p>
	{/if}
	{#if counted}
		<Scoreboard won={session.won} ruleset={variant.ruleset} />
	{/if}
{/snippet}

{#snippet turn()}
	{#if 'ToMove' in session.status}
		<TurnCard
			player={session.status.ToMove.player}
			sent={session.status.ToMove.sent}
			last={session.moves.at(-1)?.notation ?? null}
			refused={session.refusal !== null}
			decidedAbove={session.view.sent && decidedAbove(session.view, session.view.sent)}
		/>
	{/if}
{/snippet}

{#snippet controls()}
	<GameControls
		canUndo={session.canUndo}
		resigner={session.resigner}
		onundo={() => session.undo()}
		onresign={() => session.resign()}
	/>
{/snippet}
