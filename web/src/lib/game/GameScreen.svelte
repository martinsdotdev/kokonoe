<!--
	The game screen on desktop (spec §7.4), in the design overview's three columns, which wrap on
	narrow screens: players and the turn, the board, then the moves and the controls.
-->
<script lang="ts">
	import { resolve } from '$app/paths';
	import type { Player } from '#lib/engine.js';
	import Board from './Board.svelte';
	import GameControls from './GameControls.svelte';
	import MoveList from './MoveList.svelte';
	import PlayerBar from './PlayerBar.svelte';
	import ResultBanner from './ResultBanner.svelte';
	import RulesCard from './RulesCard.svelte';
	import Scoreboard from './Scoreboard.svelte';
	import { againstLabel } from './launch.js';
	import { resultText, winner } from './result.js';
	import type { OfflineGame } from './session.svelte.js';
	import TurnCard from './TurnCard.svelte';

	let { session }: { session: OfflineGame } = $props();

	const played = $derived(session.moves.filter((move) => !move.dealt).length);
	const opponent = $derived(session.opponent);
	const variant = $derived(session.variant);
	const against = $derived(againstLabel(opponent));
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
		<div class="mx-auto flex max-w-[1320px] flex-wrap items-center justify-between gap-4 px-7 py-3">
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
				<span class="text-muted-foreground">{against}</span>
			</div>
			<a
				href={resolve('/')}
				class="inline-flex min-h-11 items-center px-3.5 font-medium text-foreground"
			>
				Leave game
			</a>
		</div>
	</header>

	<main class="mx-auto flex max-w-[1320px] flex-col gap-6 p-7">
		<h1 class="sr-only">{variant.name}, {against}</h1>
		{#if session.end}
			<ResultBanner
				headline={resultText(session.end, variant.ruleset, session.won)}
				detail={`After ${played} ${played === 1 ? 'move' : 'moves'}.`}
				winner={winner(session.end)}
				onrestart={() => session.restart()}
			/>
		{/if}
		<div class="flex flex-wrap items-start justify-center gap-8">
			<aside
				aria-label="Players and turn"
				class="flex min-w-0 flex-[1_1_260px] flex-col gap-3.5 lg:max-w-80"
			>
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
				{#if 'ToMove' in session.status}
					<TurnCard
						player={session.status.ToMove.player}
						sent={session.status.ToMove.sent}
						last={session.moves.at(-1)?.notation ?? null}
					/>
				{/if}
			</aside>
			<section aria-label="Board" class="flex min-w-0 flex-[0_1_600px] justify-center">
				<Board
					view={session.view}
					flash={session.flash}
					disabled={session.end !== null || session.computerToMove}
					onplay={(cell) => session.play(cell)}
				/>
			</section>
			<aside aria-label="Moves" class="flex min-w-0 flex-[1_1_260px] flex-col gap-3.5 lg:max-w-80">
				<MoveList moves={session.moves} />
				<GameControls
					canUndo={session.canUndo}
					resigner={session.resigner}
					onundo={() => session.undo()}
					onresign={() => session.resign()}
				/>
				<RulesCard {variant} />
			</aside>
		</div>
	</main>
</div>
