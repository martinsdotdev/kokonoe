<!--
	The Replay viewer: a game saved on this device, `?game=<id>`, or one `/import` opened, which the
	page's history state carries so a reload in the tab keeps it. `&ply=N` is the ply shown, which
	the address keeps as it changes, so a reload stays put.
-->
<script lang="ts">
	import { onDestroy } from 'svelte';
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { page } from '$app/state';
	import { loadEngine } from '#lib/engine.js';
	import { replayQuery } from '#lib/game/launch.js';
	import PageProblem from '#lib/game/PageProblem.svelte';
	import { ENGINE, explain, failed, Problem } from '#lib/game/problem.js';
	import { loadSaved, UnplayableGame } from '#lib/game/record.js';
	import { fromSaved, Replay, type ReplaySource } from '#lib/game/replay.svelte.js';
	import ReplayScreen from '#lib/game/ReplayScreen.svelte';

	let replay: Replay | null = $state(null);
	let problem: string | null = $state(null);
	/** The page was left, perhaps before the game loaded. */
	let left = false;

	// The address itself, not `page.url`: a shallow step leaves `page.url` behind, and Back
	// remounts the page from it, without the ply.
	const params = new URLSearchParams(location.search);
	const id = params.get('game');

	/** The game the address or the history state names. */
	const source = async (): Promise<ReplaySource> => {
		if (id !== null) return fromSaved(await loadSaved(id));
		const imported = page.state.imported;
		if (!imported) throw new Problem('This link names no game to replay.');
		await loadEngine().catch(failed(ENGINE));
		return imported;
	};

	const open = async (): Promise<Replay | null> => {
		const game = await source();
		if (left) return null;
		try {
			const opened = Replay.open(game);
			opened.go(Number.parseInt(params.get('ply') ?? '0', 10) || 0);
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

	// The address keeps the ply shown, and the history state the imported game.
	$effect(() => {
		if (!replay) return;
		const query = replayQuery(id, replay.ply);
		if (location.search === query) return;
		const state = page.state;
		goto(resolve(`/replay${query}`), { shallow: true, replace: true, state, persistState: true });
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
