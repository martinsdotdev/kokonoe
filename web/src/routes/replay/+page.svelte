<!--
	The Replay viewer for a game saved on this device: `?game=<id>`, with `&ply=N` for the ply
	shown, which the address keeps as it changes, so a reload stays put.
-->
<script lang="ts">
	import { onDestroy } from 'svelte';
	import { replaceState } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { page } from '$app/state';
	import { replayQuery } from '#lib/game/launch.js';
	import { UnplayableGame } from '#lib/game/record.js';
	import { Replay } from '#lib/game/replay.svelte.js';
	import ReplayScreen from '#lib/game/ReplayScreen.svelte';
	import PageProblem from '#lib/game/PageProblem.svelte';
	import { explain, loadSaved, Problem } from '#lib/game/problem.js';

	let replay: Replay | null = $state(null);
	let problem: string | null = $state(null);
	/** The page was left, perhaps before the game loaded. */
	let left = false;

	const id = page.url.searchParams.get('game');

	const open = async (): Promise<Replay | null> => {
		if (id === null) throw new Problem('This link names no game to replay.');
		const saved = await loadSaved(id);
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
		(error: unknown) => (problem = explain(error, 'The replay couldn’t open.'))
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
	<PageProblem {problem} title="This replay can’t open" to="/" link="Back to Home" />
{/if}
