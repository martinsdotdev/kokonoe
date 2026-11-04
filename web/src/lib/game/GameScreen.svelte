<!--
	The game screen (spec §7.4). Wider than a phone it has the design overview's three columns,
	which wrap on narrower screens: players and the turn, the board, then the moves and the
	controls. On a phone the board comes first and steps into one Board at a time (§7.3), with the
	turn under it, then the players, the controls, the moves and the rules, in that order in the
	page as on screen. Deep steps on every screen: its Cells are too small to play on the whole
	board.

	With keyboard focus on the board, a Cell can be typed as its Notation digits (spec §7.7). One
	live line, mounted for the whole game, says each move, where the next goes, and the result, and
	in a timed Room this page's first-move window and its clock running low. In a Room it says the
	other player leaving and coming back, and the Claim once it's due.

	A Spectator's page (spec §10.2) shows the game read-only: its board takes no moves, as a
	Replay's, and it has no controls, answers no offer, and claims nothing. Everyone in a Room sees
	how many watch, as plain text the live line doesn't say, so no one coming and going interrupts
	it.
-->
<script lang="ts">
	import { tick } from 'svelte';
	import type { Attachment } from 'svelte/attachments';
	import { on } from 'svelte/events';
	import { MediaQuery } from 'svelte/reactivity';
	import { announcement, turn } from './announce.js';
	import Board from './Board.svelte';
	import ClaimCard from './ClaimCard.svelte';
	import { CUES, left } from './clock.js';
	import GameControls from './GameControls.svelte';
	import MoveList from './MoveList.svelte';
	import OfferCard from './OfferCard.svelte';
	import { offerText } from './offers.js';
	import PlayerBar from './PlayerBar.svelte';
	import { claimText, UNSEATED, watchingText } from './presence.js';
	import ResultBanner from './ResultBanner.svelte';
	import RulesCard from './RulesCard.svelte';
	import Scoreboard from './Scoreboard.svelte';
	import ScreenHeader from './ScreenHeader.svelte';
	import SteppedBoard from './SteppedBoard.svelte';
	import { shortcut, typed, typedSoFar } from './keys.js';
	import { moves, seconds } from './listing.js';
	import { playedCount } from './moves.js';
	import { notation } from './notation.js';
	import { result, resultText } from './result.js';
	import { counted } from './rules.js';
	import type { GameSession } from './screen.js';
	import TurnCard from './TurnCard.svelte';
	import { decidedAbove } from './view.js';

	let { session }: { session: GameSession } = $props();

	/** Narrower than 40rem, Tailwind's `max-sm`: a phone. */
	const phone = new MediaQuery('(width < 40rem)');

	const played = $derived(playedCount(session.moves));
	const variant = $derived(session.variant);
	const against = $derived(session.against);
	const frozen = $derived(session.frozen);
	/** This page watches a Room as a Spectator: its board only shows the game. */
	const watching = $derived(session.watching === true);
	/** A Room this page watches still waits for its second player. */
	const unseated = $derived(watching && session.seated === false);
	/** The header's way out, and how the page names itself. */
	const exit = $derived(watching ? 'Stop watching' : 'Leave game');
	const title = $derived(watching ? `Watching ${against}` : variant.name);
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

	/** The offer waiting, which shows by the controls, or a rematch's in the result banner. */
	const offer = $derived(session.offer);
	const answer = (accept: boolean) => session.answer(accept);

	/** Since when each player has been away from the Room. */
	const away = $derived(session.away);
	const absent = $derived(away.X !== null || away.O !== null);

	/**
	 * A timed Room's clocks, and the time now, which ticks while one runs, a first move is awaited
	 * or a player is away.
	 */
	const clocks = $derived(session.clocks);
	/**
	 * The first move the player to move must make before the game is aborted: by when, and how
	 * long they had when the clocks were read.
	 */
	const firstMove = $derived(
		clocks && clocks.aborts !== null
			? { by: clocks.aborts, within: clocks.aborts - clocks.at }
			: null
	);
	let instant = $state(Date.now());
	$effect(() => {
		if (!clocks?.running && !firstMove && !absent) return;
		instant = Date.now();
		const ticking = setInterval(() => (instant = Date.now()), 100);
		return () => clearInterval(ticking);
	});

	/**
	 * The Claim against a player who left, once it's due: a card by the controls, said once. The
	 * same Claim from one tick to the next, so nothing that reads it changes every tick.
	 */
	const claim = $derived(session.claim);
	const due = $derived(claim && instant >= claim.due ? claim : null);
	/** What came of an offer or of the other player's presence, unless the Claim's card says it. */
	const notice = $derived(due ? null : session.notice);

	/** The seat this page plays, in a Room. */
	const mine = $derived((['X', 'O'] as const).find((player) => session.seats[player].yours));

	/** `ms` as whole seconds, rounded up. */
	const inSeconds = (ms: number) => seconds(Math.max(0, Math.ceil(ms / 1000)));

	/** How long the player to move has to make their first move, before the game is aborted. */
	const countdown = $derived.by(() => {
		const player = session.toMove;
		if (!firstMove || !player) return null;
		const time = inSeconds(firstMove.by - instant);
		return player === mine
			? `Make your first move within ${time}, or the game is aborted.`
			: `${session.seats[player].name} has ${time} to make a first move, or the game is aborted.`;
	});

	/**
	 * What follows the move on the live line: this page's first-move window as it opens, then a
	 * Claim that's due, an offer waiting, and what came of this page's offer or of the other
	 * player's presence.
	 */
	const after = $derived.by(() => {
		const window =
			firstMove && session.toMove === mine
				? `You have ${inSeconds(firstMove.within)} to make your first move.`
				: null;
		const claimed = due && claimText(due.other);
		return [window, claimed, offer && offerText(offer), notice].filter(Boolean).join(' ');
	});

	/** This page's clock running low, said until anything else is: a change to either clears it. */
	let cue = $derived.by((): string | null => {
		void said;
		void after;
		return null;
	});
	/**
	 * The lowest of the `CUES` this page's clock was under when last read, which only the effect
	 * reads. A cue is said each time the clock falls past it, as it can again after an increment,
	 * but not for one it was already past when the page opened.
	 */
	let cued: number | undefined;
	$effect(() => {
		if (!clocks || !mine || session.end) return;
		const ms = left(clocks, mine, instant);
		const due = CUES.findLast((at) => ms <= at) ?? Infinity;
		if (cued !== undefined && due < cued) cue = `${due / 1000} seconds left.`;
		cued = due;
	});

	/**
	 * The game's one live line: what keeps the game from going on, or this page's clock running
	 * low, or the last move, then the result or where to play, then what follows it.
	 */
	const spoken = $derived.by(() => {
		if (session.trouble) return session.trouble.message;
		if (unseated) return UNSEATED;
		return cue ?? (after ? `${said} ${after}`.trim() : said);
	});

	let heading: HTMLElement | undefined = $state();
	/**
	 * A card goes, an offer's once it's answered, lapses or is withdrawn, and a Claim's once it's
	 * made or the other player is back, and takes focus with it if it held it: focus goes to the
	 * game, as when the page swaps one view for another.
	 */
	const refocusWhenGone = (shown: () => boolean) => {
		let was = false;
		$effect(() => {
			const now = shown();
			if (was && !now) {
				tick().then(() => {
					if (document.activeElement === document.body) heading?.focus();
				});
			}
			was = now;
		});
	};
	refocusWhenGone(() => offer !== null);
	refocusWhenGone(() => due !== null);

	/** The last move, then the result or where to play. */
	const said = $derived.by(() => {
		const last = session.moves.at(-1);
		if (session.end) {
			// A game that stopped ended on no move, and the last move was said when it was made.
			const made = 'outcome' in session.end ? last : undefined;
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
</script>

<svelte:head>
	<title>{title} · Kokonoe</title>
</svelte:head>

<div class="min-h-svh">
	<ScreenHeader variant={variant.name} {against} {exit} />

	<main class="mx-auto flex max-w-[1320px] flex-col gap-4 p-4 sm:gap-6 sm:p-7">
		<!-- Focusable, so a page that swaps a game in can move focus to it. -->
		<h1 class="sr-only" tabindex="-1" bind:this={heading}>
			{watching ? 'Watching ' : ''}{variant.name}, {against}
		</h1>
		<p class="sr-only" aria-live="polite" data-spoken>{spoken}</p>
		{#if session.end}
			<ResultBanner
				headline={resultText(session.end, variant.ruleset, session.won)}
				detail={`After ${moves(played)}.`}
				result={result(session.end)}
				game={session.replay}
				file={session.file}
				onrestart={session.restart && (() => session.restart?.())}
				rematch={session.rematch
					? {
							offer: session.rematch,
							waiting: offer?.kind === 'Rematch' ? offer : null,
							onanswer: answer,
							room: session.rematchRoom
						}
					: watching
						? { waiting: offer?.kind === 'Rematch' ? offer : null, room: session.rematchRoom }
						: undefined}
			/>
		{/if}
		{#if phone.current}
			<section aria-label="Board" class="flex justify-center" {@attach typing}>
				{@render board()}
			</section>
			{@render turnCard()}
			<!-- Before the players, so an offer, which may lapse in seconds, isn't off the screen. -->
			{@render offers()}
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
					{@render offers()}
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
			onplay={watching ? undefined : (cell) => session.play(cell)}
		/>
	{:else}
		<Board
			view={session.view}
			flash={session.flash}
			disabled={frozen}
			onplay={watching ? undefined : (cell) => session.play(cell)}
		/>
	{/if}
{/snippet}

{#snippet players()}
	{#each ['X', 'O'] as const as player (player)}
		{@const since = away[player]}
		<PlayerBar
			{player}
			seat={session.seats[player]}
			toMove={!session.end && session.toMove === player}
			doing={session.doing(player)}
			clock={clocks && left(clocks, player, instant)}
			away={since === null ? null : Math.max(0, instant - since)}
		/>
	{/each}
	{#if watching || session.spectators}
		<p class="m-0 text-sm text-muted-foreground" data-watching>
			{watchingText(session.spectators ?? 0, watching)}
		</p>
	{/if}
	{#if session.trouble}
		{@const { message, retry } = session.trouble}
		<p class="m-0 flex flex-wrap items-center gap-3 text-sm" data-trouble>
			{message}
			{#if retry}
				<button
					type="button"
					class="min-h-11 rounded-md border-[1.5px] border-foreground bg-card px-4 font-semibold"
					onclick={retry}
				>
					Try again
				</button>
			{/if}
		</p>
	{/if}
	{#if counted(variant.ruleset)}
		<Scoreboard won={session.won} ruleset={variant.ruleset} />
	{/if}
{/snippet}

{#snippet turnCard()}
	{#if now && session.toMove && !unseated}
		<TurnCard
			player={session.toMove}
			turn={now}
			typed={digits ? typedSoFar(digits, depth) : null}
			{countdown}
		/>
	{/if}
{/snippet}

{#snippet offers()}
	{#if due}
		<ClaimCard claim={due} />
	{/if}
	{#if offer && offer.kind !== 'Rematch'}
		<OfferCard {offer} onanswer={watching ? undefined : answer} />
	{/if}
	{#if unseated}
		<p class="m-0 text-sm" data-unseated>{UNSEATED}</p>
	{/if}
	{#if notice}
		<p class="m-0 text-sm" data-notice>{notice}</p>
	{/if}
{/snippet}

{#snippet controls()}
	{#if session.controls}
		<GameControls controls={session.controls} />
	{/if}
{/snippet}
