<!-- A game page until its game opens: loading the engine, or why it can't open and where to go. -->
<script lang="ts">
	import { resolve } from '$app/paths';

	interface Props {
		/** Why the page can't open, or null while it loads. */
		problem: string | null;
		title: string;
		/** Where to go instead. */
		to: '/' | '/new-game';
		link: string;
	}

	let { problem, title, to, link }: Props = $props();
</script>

<svelte:head>
	<title>{problem ? `${title} · Kokonoe` : 'Kokonoe'}</title>
</svelte:head>

<main class="mx-auto flex min-h-svh max-w-xl flex-col justify-center gap-3 px-4">
	{#if problem}
		<h1 class="font-display text-3xl font-semibold">{title}</h1>
		<p class="text-muted-foreground">{problem}</p>
		<a href={resolve(to)} class="font-semibold text-foreground">{link}</a>
	{:else}
		<p class="text-muted-foreground">Loading the engine…</p>
	{/if}
</main>
