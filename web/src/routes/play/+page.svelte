<!--
	The game screen. Its URL names the game (see launch.ts): the variant by its Ruleset code, and
	the opponent, so a reload restarts the same game.
-->
<script lang="ts">
	import { onDestroy } from 'svelte';
	import { resolve } from '$app/paths';
	import { page } from '$app/state';
	import { loadEngine } from '#lib/engine.js';
	import GameScreen from '#lib/game/GameScreen.svelte';
	import { readLaunch } from '#lib/game/launch.js';
	import { OfflineGame } from '#lib/game/session.svelte.js';
	import { presetByCode } from '#lib/game/shelf.js';

	let session: OfflineGame | null = $state(null);
	let problem: string | null = $state(null);

	const launch = readLaunch(page.url.searchParams);
	const preset = 'error' in launch ? undefined : presetByCode(launch.code);

	if ('error' in launch) {
		problem = launch.error;
	} else if (!preset) {
		problem = `Kokonoe doesn’t know the variant “${launch.code}”.`;
	} else if (!preset.playable) {
		problem = `${preset.name} arrives soon.`;
	} else {
		const { opponent } = launch;
		loadEngine().then(
			() => (session = new OfflineGame(preset, opponent)),
			(error: unknown) => {
				// The page only says it failed, so say why where a developer can see it.
				console.error('The engine failed to load.', error);
				problem = 'The engine didn’t load. Reload the page to try again.';
			}
		);
	}
	onDestroy(() => session?.dispose());
</script>

{#if session}
	<GameScreen {session} />
{:else}
	<main class="mx-auto flex min-h-svh max-w-xl flex-col justify-center gap-3 px-4">
		{#if problem}
			<h1 class="font-display text-3xl font-semibold">This game can’t start</h1>
			<p class="text-muted-foreground">{problem}</p>
			<a href={resolve('/new-game')} class="font-semibold text-foreground">Choose a new game</a>
		{:else}
			<p class="text-muted-foreground">Loading the engine…</p>
		{/if}
	</main>
{/if}
