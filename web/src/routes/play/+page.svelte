<!--
	The game screen. Its URL names the game (see launch.ts): a new one by its Ruleset code and
	opponent, or a saved one by its id. Once a game is saved the address becomes its id, so a reload
	resumes it.
-->
<script lang="ts">
	import { onDestroy } from 'svelte';
	import { replaceState } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { page } from '$app/state';
	import { loadEngine } from '#lib/engine.js';
	import GameScreen from '#lib/game/GameScreen.svelte';
	import { gameQuery, playQuery, readLaunch } from '#lib/game/launch.js';
	import { rulesetCode } from '#lib/game/rules.js';
	import { loadGame } from '#lib/game/saved.js';
	import { OfflineGame } from '#lib/game/session.svelte.js';
	import { presetByCode } from '#lib/game/shelf.js';

	let session: OfflineGame | null = $state(null);
	let problem: string | null = $state(null);
	/** The page was left, perhaps before the engine loaded. */
	let left = false;

	/** Shows the game `make` builds, unless the page was left meanwhile. */
	const start = (make: () => OfflineGame) => {
		// A computer on X would start a search that nothing stops.
		if (left) return;
		try {
			session = make();
		} catch (error) {
			problem = `This game can’t be resumed. ${error instanceof Error ? error.message : error}`;
		}
	};

	const engineFailed = (error: unknown) => {
		// The page only says it failed, so say why where a developer can see it.
		console.error('The engine failed to load.', error);
		problem = 'The engine didn’t load. Reload the page to try again.';
	};

	const id = page.url.searchParams.get('game');
	if (id !== null) {
		const saved = loadGame(id).catch((error: unknown) => {
			console.error('The saved game couldn’t be read.', error);
			problem = 'Saved games can’t be read in this browser.';
		});
		Promise.all([loadEngine(), saved]).then(([, saved]) => {
			if (saved) start(() => OfflineGame.resume(saved));
			else problem ??= 'This game isn’t saved on this device.';
		}, engineFailed);
	} else {
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
			loadEngine().then(() => start(() => new OfflineGame(preset, opponent)), engineFailed);
		}
	}

	// The address follows the game: its id once saved, or the new game a restart begins.
	$effect(() => {
		if (!session) return;
		const query = session.stored
			? gameQuery(session.id)
			: playQuery({ code: rulesetCode(session.variant.ruleset), opponent: session.opponent });
		if (location.search !== query) replaceState(resolve(`/play${query}`), {});
	});

	onDestroy(() => {
		left = true;
		session?.dispose();
	});
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
