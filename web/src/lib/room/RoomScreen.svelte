<!--
	A Room's link, `/r/<room>` (spec §10.1): the joiner's card, the host's wait for a friend, then
	the game screen, which says when the connection drops; or why the game can't be played here.
	Each time the page swaps one view for another, focus moves to the new view's heading, so a
	screen reader hears where the page went.
-->
<script lang="ts">
	import { onDestroy, tick } from 'svelte';
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import Brand from '#lib/components/Brand.svelte';
	import GameScreen from '#lib/game/GameScreen.svelte';
	import { replayQuery } from '#lib/game/launch.js';
	import { moves, turnLine } from '#lib/game/listing.js';
	import MiniBoard from '#lib/game/MiniBoard.svelte';
	import JoinCard from './JoinCard.svelte';
	import { RoomVisit, type VisitOptions } from './visit.svelte.js';
	import WaitingCard from './WaitingCard.svelte';

	/** `options` is for tests, which fake the server. */
	let { room, options }: { room: string; options?: VisitOptions } = $props();

	// The route keys this component by the Room, so neither prop changes under it.
	// svelte-ignore state_referenced_locally
	const visit = new RoomVisit(room, {
		follow: (next) => goto(resolve('/r/[room]', { room: next })),
		...options
	});
	onDestroy(() => visit.dispose());

	const phase = $derived(visit.phase);
	/**
	 * The host's game, and their seat, before a friend takes the other seat. A Spectator watches
	 * the game screen instead.
	 */
	const waiting = $derived.by(() => {
		if (phase.kind !== 'playing' || phase.game.seated) return null;
		const { game } = phase;
		return game.you ? { game, you: game.you } : null;
	});
	/** Which view the page shows. */
	const view = $derived(phase.kind === 'playing' ? (waiting ? 'waiting' : 'game') : phase.kind);

	let shown: string | null = null;
	$effect(() => {
		const now = view;
		if (shown !== null && shown !== now) {
			tick().then(() => document.querySelector<HTMLElement>('main h1')?.focus());
		}
		shown = now;
	});

	/** What the page says when there's no game to show: a heading, and why. */
	const notice = $derived.by((): { heading: string; text?: string } | null => {
		switch (phase.kind) {
			case 'loading':
			case 'connecting':
				return { heading: 'Opening the game…' };
			case 'offline':
				return {
					heading: 'You’re offline',
					text: 'An invite Room is played on Kokonoe’s server, which this page can’t reach. It tries again when you’re back online.'
				};
			case 'missing':
				return {
					heading: 'This game doesn’t exist',
					text: 'Check the link you were sent: it may be mistyped.'
				};
			case 'aborted':
				return {
					heading: 'Game aborted',
					text: `${phase.late ?? 'A player'} didn’t make a first move in time, so the game wasn’t kept.`
				};
			case 'full':
				return {
					heading: 'This game is full',
					text: `${phase.players.X} and ${phase.players.O} are playing it.`
				};
			case 'update':
				return {
					heading: 'A newer Kokonoe is out',
					text: 'This game is played with it. Reload the page to get it.'
				};
			case 'problem':
				return { heading: 'This game can’t open', text: phase.message };
			default:
				return null;
		}
	});

	const button =
		'min-h-12 self-start rounded-md border-[1.5px] border-foreground bg-card px-5 font-semibold';
</script>

<svelte:head>
	{#if phase.kind === 'join'}
		<title>{phase.card.host} invites you · Kokonoe</title>
	{:else if waiting}
		<title>Waiting for your friend · Kokonoe</title>
	{:else if notice}
		<title>{notice.text ? `${notice.heading} · Kokonoe` : 'Kokonoe'}</title>
	{/if}
</svelte:head>

{#if phase.kind === 'playing' && !waiting}
	<GameScreen session={phase.game} />
{:else}
	<div class="min-h-svh">
		<header class="mx-auto flex max-w-[1240px] items-center px-4 py-6 sm:px-8">
			<Brand />
		</header>
		<main class="mx-auto flex max-w-xl flex-col gap-4 px-4 pb-12" data-room={phase.kind}>
			{#if phase.kind === 'join'}
				<JoinCard card={phase.card} onaccept={() => visit.accept()} />
			{:else if waiting}
				<WaitingCard
					link={`${location.origin}${resolve('/r/[room]', { room })}`}
					variant={waiting.game.variant.name}
					you={waiting.you}
					clock={waiting.game.clock}
					trouble={waiting.game.trouble?.message ?? null}
				/>
			{:else if notice}
				<h1 class="m-0 font-display text-3xl font-semibold" tabindex="-1">{notice.heading}</h1>
				{#if notice.text}
					<p class="m-0 text-muted-foreground">{notice.text}</p>
				{/if}
				{#if phase.kind === 'offline'}
					{#if phase.saved}
						{@const saved = phase.saved}
						<figure class="m-0 flex items-center gap-4.5">
							<div class="w-[120px] shrink-0">
								<MiniBoard
									view={saved.view}
									label={`The board as you last saw it, after ${moves(saved.record.moves.length)}`}
								/>
							</div>
							<figcaption class="font-semibold">{saved.result ?? turnLine(saved)}</figcaption>
						</figure>
						{#if saved.result}
							<a
								href={resolve(`/replay${replayQuery(saved.id)}`)}
								class="font-semibold text-foreground"
							>
								Replay this game
							</a>
						{/if}
					{/if}
					<button type="button" class={button} onclick={() => visit.retry()}>Try again</button>
				{:else if phase.kind === 'update'}
					<button type="button" class={button} onclick={() => location.reload()}>Reload</button>
				{:else if notice.text}
					<a href={resolve('/')} class="font-semibold text-foreground">Go to Home</a>
				{/if}
			{/if}
		</main>
	</div>
{/if}
