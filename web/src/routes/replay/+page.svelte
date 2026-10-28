<!--
	The Replay viewer for a game saved on this device: `?game=<id>`, with `&ply=N` for the ply
	shown, which the address keeps as it changes, so a reload stays put.
-->
<script lang="ts">
	import { onDestroy } from 'svelte';
	import { replaceState } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { page } from '$app/state';
	import { loadEngine } from '#lib/engine.js';
	import { replayQuery } from '#lib/game/launch.js';
	import { UnplayableGame } from '#lib/game/record.js';
	import { Replay } from '#lib/game/replay.svelte.js';
	import ReplayScreen from '#lib/game/ReplayScreen.svelte';
	import { loadGame } from '#lib/game/saved.js';

	let replay: Replay | null = $state(null);
	let problem: string | null = $state(null);
	/** The page was left, perhaps before the game loaded. */
	let left = false;

	/** A failure the page explains, rather than only logging it. */
	class Problem extends Error {}

	/** Throws `message` for the page to show, with the cause logged where a developer can see it. */
	const failed = (message: string) => (error: unknown) => {
		console.error(message, error);
		throw new Problem(message);
	};

	const id = page.url.searchParams.get('game');

	const open = async (): Promise<Replay | null> => {
		if (id === null) throw new Problem('This link names no game to replay.');
		const [saved] = await Promise.all([
			loadGame(id).catch(failed('Saved games can’t be read in this browser.')),
			loadEngine().catch(failed('The engine didn’t load. Reload the page to try again.'))
		]);
		if (!saved) throw new Problem('This game isn’t saved on this device.');
		if (left) return null;
		try {
			const opened = Replay.open(saved);
			opened.go(Number.parseInt(page.url.searchParams.get('ply') ?? '0', 10) || 0);
			return opened;
		} catch (error) {
			if (error instanceof UnplayableGame) {
				throw new Problem(`This game can’t be replayed. ${error.message}`);
			}
			throw error;
		}
	};

	open().then(
		(opened) => (replay = opened),
		(error: unknown) => {
			if (!(error instanceof Problem)) console.error('The replay couldn’t open.', error);
			problem =
				error instanceof Problem
					? error.message
					: 'Something went wrong. Reload the page to try again.';
		}
	);

	// The address keeps the ply shown.
	$effect(() => {
		if (!replay || id === null) return;
		const query = replayQuery(id, replay.ply);
		if (location.search !== query) replaceState(resolve(`/replay${query}`), {});
	});

	onDestroy(() => {
		left = true;
		replay?.dispose();
	});
</script>

{#if replay}
	<ReplayScreen {replay} />
{:else}
	<main class="mx-auto flex min-h-svh max-w-xl flex-col justify-center gap-3 px-4">
		{#if problem}
			<h1 class="font-display text-3xl font-semibold">This replay can’t open</h1>
			<p class="text-muted-foreground">{problem}</p>
			<a href={resolve('/')} class="font-semibold text-foreground">Back to Home</a>
		{:else}
			<p class="text-muted-foreground">Loading the engine…</p>
		{/if}
	</main>
{/if}
