<!--
	The game screen (spec §7.4). Wider than a phone it has the design overview's three columns,
	which wrap on narrower screens: players and the turn, the board, then the moves and the
	controls. On a phone the board comes first and steps into one Board at a time (§7.3), with the
	turn under it, then the players, the controls, the moves and the rules, in that order in the
	page as on screen. Deep steps on every screen: its Cells are too small to play on the whole
	board.

	With keyboard focus on the board, a Cell can be typed as its Notation digits (spec §7.7). One
	live line, mounted for the whole game, says each move, where the next goes, and the result.
-->
<script lang="ts">
	import type { Attachment } from 'svelte/attachments';
	import { on } from 'svelte/events';
	import { MediaQuery } from 'svelte/reactivity';
	import type { Player } from '#lib/engine.js';
	import { announcement, turn } from './announce.js';
	import Board from './Board.svelte';
	import GameControls from './GameControls.svelte';
	import MoveList from './MoveList.svelte';
	import PlayerBar from './PlayerBar.svelte';
	import ResultBanner from './ResultBanner.svelte';
	import RulesCard from './RulesCard.svelte';
	import Scoreboard from './Scoreboard.svelte';
	import ScreenHeader from './ScreenHeader.svelte';
	import SteppedBoard from './SteppedBoard.svelte';
	import { shortcut, typed, typedSoFar } from './keys.js';
	import { againstLabel } from './launch.js';
	import { moves } from './listing.js';
	import { playedCount } from './moves.js';
	import { notation } from './notation.js';
	import { resultText, winner } from './result.js';
	import { counted } from './rules.js';
	import type { OfflineGame } from './session.svelte.js';
	import TurnCard from './TurnCard.svelte';
	import { decidedAbove } from './view.js';

	let { session }: { session: OfflineGame } = $props();

	/** Narrower than 40rem, Tailwind's `max-sm`: a phone. */
	const phone = new MediaQuery('(width < 40rem)');

	const played = $derived(playedCount(session.moves));
	const opponent = $derived(session.opponent);
	const variant = $derived(session.variant);
	const against = $derived(againstLabel(opponent));
	/** The board takes no moves: the game is over, or the computer is to move. */
	const frozen = $derived(session.end !== null || session.computerToMove);
	/** One Board at a time: on a phone, and at depth 3 everywhere. */
	const stepped = $derived(phone.current || variant.ruleset.depth === 3);

	const depth = $derived(variant.ruleset.depth);

	/** Where the player to move may play and why, while the game goes on. */
	const now = $derived.by(() => {
		if (session.end || !('ToMove' in session.status)) return null;
		const { player, sent } = session.status.ToMove;
		const { view, refusal } = session;
		return turn(
			player,
			sent,
			session.moves.at(-1)?.notation ?? null,
			view.sent && decidedAbove(view, view.sent),
			refusal && notation(refusal.cell, depth)
		);
	});

	/** The game's one live line: the last move, then the result or where to play. */
	const spoken = $derived.by(() => {
		const last = session.moves.at(-1);
		if (session.end) {
			// A resignation is no move, and the last move was said when it was made.
			const made = 'resigned' in session.end ? undefined : last;
			return announcement(made, resultText(session.end, variant.ruleset, session.won));
		}
		if (!now) return '';
		// The same refusal twice reads the same, which a live region wouldn't say again, so every
		// other one ends in a space that isn't read.
		const again = (session.refusal?.tick ?? 0) % 2 ? '\u00a0' : '';
		return announcement(last, now.line) + again;
	});

	/** The digits of a Cell being typed. Any change to the game clears them, so none carry over. */
	let digits = $derived.by(() => {
		void session.moves;
		void frozen;
		return '';
	});

	const onkeydown = (event: KeyboardEvent) => {
		if (frozen || event.repeat || !shortcut(event)) return;
		const next = typed(digits, event.key, depth);
		if (!next) return;
		event.preventDefault();
		digits = next.digits;
		if (next.cell !== undefined) session.play(next.cell);
	};

	/**
	 * Typing a Cell, only while keyboard focus is on the board: digits that played from anywhere
	 * on the page would be single-key shortcuts, which WCAG 2.1.4 rules out.
	 */
	const typing: Attachment<HTMLElement> = (board) => on(board, 'keydown', onkeydown);

	/** What the computer is doing on its move, for its player bar. */
	const doing = (player: Player) =>
		session.computerToMove && session.toMove === player
			? session.stuck
				? 'Couldn’t move'
				: 'Thinking…'
			: null;
</script>

<svelte:head>
	<title>{variant.name} · Kokonoe</title>
</svelte:head>

<div class="min-h-svh">
	<ScreenHeader variant={variant.name} {against} exit="Leave game" />

	<main class="mx-auto flex max-w-[1320px] flex-col gap-4 p-4 sm:gap-6 sm:p-7">
		<h1 class="sr-only">{variant.name}, {against}</h1>
		<p class="sr-only" aria-live="polite" data-spoken>{spoken}</p>
		{#if session.end}
			<ResultBanner
				headline={resultText(session.end, variant.ruleset, session.won)}
				detail={`After ${moves(played)}.`}
				winner={winner(session.end)}
				game={session.stored ? session.id : undefined}
				file={session.file}
				onrestart={() => session.restart()}
			/>
		{/if}
		{#if phone.current}
			<section aria-label="Board" class="flex justify-center" {@attach typing}>
				{@render board()}
			</section>
			{@render turnCard()}
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
					{@render turnCard()}
				</aside>
				<section
					aria-label="Board"
					class="flex min-w-0 flex-[0_1_600px] justify-center"
					{@attach typing}
				>
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
	{#if counted(variant.ruleset)}
		<Scoreboard won={session.won} ruleset={variant.ruleset} />
	{/if}
{/snippet}

{#snippet turnCard()}
	{#if now && session.toMove}
		<TurnCard
			player={session.toMove}
			turn={now}
			typed={digits ? typedSoFar(digits, depth) : null}
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
