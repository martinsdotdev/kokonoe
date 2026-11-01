<!--
	A Room's link, `/r/<room>` (spec §10.1): the joiner's card, the host's wait for a friend, then
	the game screen, which says when the connection drops; or why the game can't be played here.
-->
<script lang="ts">
	import { onDestroy } from 'svelte';
	import { resolve } from '$app/paths';
	import Brand from '#lib/components/Brand.svelte';
	import GameScreen from '#lib/game/GameScreen.svelte';
	import JoinCard from './JoinCard.svelte';
	import { RoomVisit } from './visit.svelte.js';
	import WaitingCard from './WaitingCard.svelte';

	let { room }: { room: string } = $props();

	// The route keys this component by the Room, so `room` never changes under it.
	// svelte-ignore state_referenced_locally
	const visit = new RoomVisit(room);
	onDestroy(() => visit.dispose());

	const phase = $derived(visit.phase);
	/** The host's game before a friend takes the other seat. */
	const waiting = $derived(phase.kind === 'playing' && !phase.game.seated ? phase.game : null);

	/** What the page says when there's no game to show. */
	const notice = $derived.by(() => {
		switch (phase.kind) {
			case 'loading':
			case 'connecting':
				return { title: null, text: 'Opening the game…' };
			case 'offline':
				return {
					title: 'You’re offline',
					text: 'An invite Room is played on Kokonoe’s server, which this page can’t reach. It tries again when you’re back online.'
				};
			case 'missing':
				return {
					title: 'This game doesn’t exist',
					text: 'Check the link you were sent: it may be mistyped.'
				};
			case 'full':
				return {
					title: 'This game is full',
					text: `${phase.players.X} and ${phase.players.O} are playing it.`
				};
			case 'update':
				return {
					title: 'A newer Kokonoe is out',
					text: 'This game is played with it. Reload the page to get it.'
				};
			case 'problem':
				return { title: 'This game can’t open', text: phase.message };
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
		<title>{notice.title ? `${notice.title} · Kokonoe` : 'Kokonoe'}</title>
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
					variant={waiting.variant.name}
					you={waiting.you}
				/>
			{:else if notice}
				{#if notice.title}
					<h1 class="m-0 font-display text-3xl font-semibold">{notice.title}</h1>
				{/if}
				<p class="m-0 text-muted-foreground">{notice.text}</p>
				{#if phase.kind === 'offline'}
					<button type="button" class={button} onclick={() => visit.retry()}>Try again</button>
				{:else if phase.kind === 'update'}
					<button type="button" class={button} onclick={() => location.reload()}>Reload</button>
				{:else if notice.title}
					<a href={resolve('/')} class="font-semibold text-foreground">Go to Home</a>
				{/if}
			{/if}
		</main>
	</div>
{/if}
